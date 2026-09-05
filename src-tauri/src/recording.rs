//! Protocol-independent, bounded recording. Producers never wait for disk I/O.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
    sync::{
        mpsc::{self, SyncSender, TrySendError},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tauri::State;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub sequence: u64,
    pub timestamp_us: u64,
    pub elapsed_us: u64,
    pub source: String,
    pub protocol: String,
    pub direction: String,
    pub transaction: u64,
    pub bytes: Vec<u8>,
    pub detail: String,
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStatus {
    pub active: bool,
    pub path: String,
    pub accepted: u64,
    pub written: u64,
    pub dropped: u64,
    pub unwritten: u64,
    pub bytes: u64,
    pub error: Option<String>,
}
struct Run {
    sender: SyncSender<Record>,
    worker: JoinHandle<()>,
    started: Instant,
}
#[derive(Clone)]
pub struct Recorder {
    run: Arc<Mutex<Option<Run>>>,
    transition: Arc<Mutex<()>>,
    status: Arc<Mutex<RecordingStatus>>,
    root: PathBuf,
}
pub fn timestamp_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}
impl Recorder {
    pub fn new(root: PathBuf) -> Self {
        Self {
            run: Arc::default(),
            transition: Arc::default(),
            status: Arc::default(),
            root,
        }
    }
    pub fn status(&self) -> RecordingStatus {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn start(&self, limit_mb: u64) -> Result<RecordingStatus, String> {
        if !(16..=16384).contains(&limit_mb) {
            return Err("录制容量须为 16～16384 MiB".into());
        }
        let _transition = self.transition.lock().map_err(|_| "录制生命周期锁损坏")?;
        let mut slot = self.run.lock().map_err(|_| "录制锁损坏")?;
        if slot.is_some() {
            return Err("请先停止上一录制会话".into());
        }
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let path = self.root.join(format!("session-{}", timestamp_us()));
        fs::create_dir(&path).map_err(|e| e.to_string())?;
        let metadata = serde_json::json!({"format":"servo-recording", "version":1, "timestampUnit":"microseconds", "limitMiB":limit_mb,
            "scope":"application I/O and explicitly selected network capture; cleared/unobserved bytes cannot be recovered"});
        fs::write(path.join("session.json"), metadata.to_string()).map_err(|e| e.to_string())?;
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path.join("records-0001.jsonl"))
            .map_err(|e| e.to_string())?;
        let (sender, receiver) = mpsc::sync_channel::<Record>(4096);
        *self.status.lock().map_err(|_| "录制状态锁损坏")? = RecordingStatus {
            active: true,
            path: path.to_string_lossy().into(),
            ..Default::default()
        };
        let status = self.status.clone();
        let worker = std::thread::spawn(move || {
            let result = (|| -> Result<(), String> {
                let mut writer = BufWriter::new(file);
                let mut part = 1;
                let mut part_bytes = 0_u64;
                let mut total = 0_u64;
                loop {
                    let record = match receiver.recv_timeout(std::time::Duration::from_millis(250))
                    {
                        Ok(r) => r,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            writer.flush().map_err(|e| e.to_string())?;
                            continue;
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    let mut line = serde_json::to_vec(&record).map_err(|e| e.to_string())?;
                    line.push(b'\n');
                    if total + line.len() as u64 > limit_mb * 1024 * 1024 {
                        writer.flush().map_err(|e| e.to_string())?;
                        writer.get_ref().sync_all().map_err(|e| e.to_string())?;
                        return Err("达到容量上限，录制已停止；后续通信不再保存".into());
                    }
                    if part_bytes > 0 && part_bytes + line.len() as u64 > 16 * 1024 * 1024 {
                        writer.flush().map_err(|e| e.to_string())?;
                        writer.get_ref().sync_all().map_err(|e| e.to_string())?;
                        part += 1;
                        part_bytes = 0;
                        writer = BufWriter::new(
                            OpenOptions::new()
                                .write(true)
                                .create_new(true)
                                .open(path.join(format!("records-{part:04}.jsonl")))
                                .map_err(|e| e.to_string())?,
                        );
                    }
                    writer.write_all(&line).map_err(|e| e.to_string())?;
                    total += line.len() as u64;
                    part_bytes += line.len() as u64;
                    let mut s = status.lock().map_err(|_| "录制状态锁损坏")?;
                    s.written += 1;
                    s.bytes = total;
                }
                writer.flush().map_err(|e| e.to_string())?;
                writer.get_ref().sync_all().map_err(|e| e.to_string())?;
                Ok(())
            })();
            let mut s = status.lock().unwrap_or_else(|e| e.into_inner());
            s.active = false;
            if let Err(error) = result {
                s.error = Some(error);
            }
            s.unwritten = s.accepted.saturating_sub(s.written);
            let summary = s.clone();
            drop(s);
            if let Ok(json) = serde_json::to_vec_pretty(&summary) {
                let _ = fs::write(path.join("summary.json"), json);
            }
        });
        *slot = Some(Run {
            sender,
            worker,
            started: Instant::now(),
        });
        Ok(self.status())
    }
    pub fn stop(&self) -> Result<RecordingStatus, String> {
        let _transition = self.transition.lock().map_err(|_| "录制生命周期锁损坏")?;
        let run = self.run.lock().map_err(|_| "录制锁损坏")?.take();
        if let Some(run) = run {
            drop(run.sender);
            run.worker.join().map_err(|_| "录制线程异常退出")?;
        }
        Ok(self.status())
    }
    pub fn emit(
        &self,
        source: &str,
        protocol: &str,
        direction: &str,
        transaction: u64,
        bytes: &[u8],
        detail: &str,
    ) {
        self.emit_at(
            source,
            protocol,
            direction,
            transaction,
            bytes,
            detail,
            timestamp_us(),
        );
    }
    #[allow(clippy::too_many_arguments)]
    pub fn emit_at(
        &self,
        source: &str,
        protocol: &str,
        direction: &str,
        transaction: u64,
        bytes: &[u8],
        detail: &str,
        timestamp_us: u64,
    ) {
        let Ok(slot) = self.run.lock() else { return };
        let Some(run) = slot.as_ref() else { return };
        let mut s = self.status.lock().unwrap_or_else(|e| e.into_inner());
        if !s.active {
            return;
        }
        let sequence = s.accepted + s.dropped + 1;
        let record = Record {
            sequence,
            timestamp_us,
            elapsed_us: run.started.elapsed().as_micros() as u64,
            source: source.into(),
            protocol: protocol.into(),
            direction: direction.into(),
            transaction,
            bytes: bytes.into(),
            detail: detail.into(),
        };
        match run.sender.try_send(record) {
            Ok(()) => s.accepted += 1,
            Err(TrySendError::Full(_)) => s.dropped += 1,
            Err(TrySendError::Disconnected(_)) => {
                s.dropped += 1;
                s.active = false;
                s.error.get_or_insert("录制线程已停止".into());
            }
        }
    }
    pub fn sessions(&self) -> Result<Vec<String>, String> {
        if !self.root.exists() {
            return Ok(vec![]);
        }
        let mut paths = fs::read_dir(&self.root)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .filter(|e| e.path().join("session.json").is_file())
            .map(|e| e.path().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        paths.sort();
        paths.reverse();
        Ok(paths)
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordPage {
    pub records: Vec<Record>,
    pub next_offset: Option<u64>,
    pub warning: Option<String>,
}
pub fn read_page(path: &str, offset: u64) -> Result<RecordPage, String> {
    let metadata_path = PathBuf::from(path).join("session.json");
    let metadata: serde_json::Value =
        serde_json::from_slice(&fs::read(metadata_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if metadata["format"] != "servo-recording" || metadata["version"] != 1 {
        return Err("录制会话格式或版本无效".into());
    }
    let summary_path = PathBuf::from(path).join("summary.json");
    let summary = fs::read(summary_path)
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
    let integrity_warning = match summary {
        None => Some("会话仍在录制或异常退出，未完成落盘确认".into()),
        Some(s)
            if !s["error"].is_null()
                || s["dropped"].as_u64().unwrap_or(0) > 0
                || s["unwritten"].as_u64().unwrap_or(0) > 0 =>
        {
            Some(format!(
                "录制存在缺口：队列丢弃={}，未写入={}，错误={}",
                s["dropped"], s["unwritten"], s["error"]
            ))
        }
        _ => None,
    };
    let mut files = fs::read_dir(path)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name().to_string_lossy().starts_with("records-")
                && e.path().extension().is_some_and(|e| e == "jsonl")
        })
        .map(|e| e.path())
        .collect::<Vec<_>>();
    files.sort();
    let mut records = Vec::new();
    let mut index = 0;
    let mut warning = integrity_warning;
    for file in files {
        let mut reader = BufReader::new(File::open(file).map_err(|e| e.to_string())?);
        loop {
            let mut line = Vec::new();
            use std::io::Read;
            let n = reader
                .by_ref()
                .take(1024 * 1024 + 1)
                .read_until(b'\n', &mut line)
                .map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            if n > 1024 * 1024 {
                return Err("录制行超过 1 MiB，文件无效".into());
            }
            if line.is_empty() {
                continue;
            }
            if index < offset {
                index += 1;
                continue;
            }
            if records.len() == 100 {
                return Ok(RecordPage {
                    records,
                    next_offset: Some(index),
                    warning,
                });
            }
            match serde_json::from_slice(&line) {
                Ok(r) => records.push(r),
                Err(_) => {
                    warning = Some("存在损坏或未完整落盘的记录，已跳过；不能视为完整录制".into())
                }
            }
            index += 1;
        }
    }
    Ok(RecordPage {
        records,
        next_offset: None,
        warning,
    })
}
#[tauri::command]
pub async fn start_recording(
    limit_mb: u64,
    state: State<'_, Recorder>,
) -> Result<RecordingStatus, String> {
    state.start(limit_mb)
}
#[tauri::command]
pub async fn stop_recording(state: State<'_, Recorder>) -> Result<RecordingStatus, String> {
    let recorder = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || recorder.stop())
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn recording_status(state: State<'_, Recorder>) -> RecordingStatus {
    state.status()
}
#[tauri::command]
pub fn recording_sessions(state: State<'_, Recorder>) -> Result<Vec<String>, String> {
    state.sessions()
}
#[tauri::command]
pub async fn recording_page(path: String, offset: u64) -> Result<RecordPage, String> {
    tauri::async_runtime::spawn_blocking(move || read_page(&path, offset))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persists_unknown_protocol_and_recovers_partial_tail() {
        let root = std::env::temp_dir().join(format!("servo-rec-{}", timestamp_us()));
        let r = Recorder::new(root.clone());
        r.start(16).unwrap();
        r.emit("test", "unknown", "rx", 1, &[0, 255], "partial");
        let status = r.stop().unwrap();
        assert_eq!(status.written, 1);
        assert!(!status.active);
        let page = read_page(&status.path, 0).unwrap();
        assert_eq!(page.records[0].bytes, [0, 255]);
        let mut file = OpenOptions::new()
            .append(true)
            .open(PathBuf::from(&status.path).join("records-0001.jsonl"))
            .unwrap();
        file.write_all(b"{broken").unwrap();
        assert!(read_page(&status.path, 0).unwrap().warning.is_some());
        fs::remove_dir_all(root).unwrap();
    }
}
