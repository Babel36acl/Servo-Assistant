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
#[derive(Clone, Default, Serialize, Deserialize)]
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
    #[serde(default)]
    pub armed: bool,
    #[serde(default)]
    pub discarded: u64,
    #[serde(default)]
    pub triggered_at_us: Option<u64>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerConfig {
    pub pre_seconds: u64,
    pub post_seconds: u64,
    pub keyword: String,
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
    context: Arc<Mutex<serde_json::Value>>,
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
            context: Arc::new(Mutex::new(
                serde_json::json!({"applicationVersion":env!("CARGO_PKG_VERSION")}),
            )),
        }
    }
    pub fn status(&self) -> RecordingStatus {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    #[cfg(test)]
    pub fn start(&self, limit_mb: u64) -> Result<RecordingStatus, String> {
        self.start_configured(limit_mb, None)
    }
    pub fn start_configured(
        &self,
        limit_mb: u64,
        trigger: Option<TriggerConfig>,
    ) -> Result<RecordingStatus, String> {
        if !(16..=16384).contains(&limit_mb) {
            return Err("录制容量须为 16～16384 MiB".into());
        }
        if trigger.as_ref().is_some_and(|t| {
            t.pre_seconds > 300
                || t.post_seconds == 0
                || t.post_seconds > 300
                || t.keyword.len() > 256
        }) {
            return Err("触发窗口须为前 0～300 秒、后 1～300 秒，关键词不超过 256 字节".into());
        }
        self.start_run(limit_mb * 1024 * 1024, 16 * 1024 * 1024, trigger)
    }
    #[cfg(test)]
    fn start_with_limits(
        &self,
        limit_bytes: u64,
        part_limit: u64,
    ) -> Result<RecordingStatus, String> {
        self.start_run(limit_bytes, part_limit, None)
    }
    pub fn context(&self) -> serde_json::Value {
        self.context
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn set_context(&self, key: &str, value: serde_json::Value) {
        let mut context = self.context.lock().unwrap_or_else(|e| e.into_inner());
        context[key] = value;
        let detail = context.to_string();
        drop(context);
        self.emit("application", "recording-context", "event", 0, &[], &detail);
    }
    fn start_run(
        &self,
        limit_bytes: u64,
        part_limit: u64,
        trigger: Option<TriggerConfig>,
    ) -> Result<RecordingStatus, String> {
        let _transition = self.transition.lock().map_err(|_| "录制生命周期锁损坏")?;
        let mut slot = self.run.lock().map_err(|_| "录制锁损坏")?;
        if slot.is_some() {
            if self.status().active {
                return Err("请先停止上一录制会话".into());
            }
            if let Some(previous) = slot.take() {
                drop(previous.sender);
                previous.worker.join().map_err(|_| "上一录制线程异常退出")?;
            }
        }
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let path = self.root.join(format!("session-{}", timestamp_us()));
        fs::create_dir(&path).map_err(|e| e.to_string())?;
        let metadata = serde_json::json!({"format":"servo-recording", "version":1, "startedAtUs":timestamp_us(), "context": self.context(), "trigger": trigger, "timestampUnit":"microseconds", "limitMiB":limit_bytes / (1024 * 1024),
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
            armed: trigger.is_some(),
            path: path.to_string_lossy().into(),
            ..Default::default()
        };
        let status = self.status.clone();
        let worker = std::thread::spawn(move || {
            let result = write_records_mode(
                receiver,
                &status,
                BufWriter::new(file),
                limit_bytes,
                part_limit,
                |part| {
                    OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(path.join(format!("records-{part:04}.jsonl")))
                        .map(BufWriter::new)
                },
                trigger,
            );
            finish_recording(&status, &path, result);
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
trait RecordOutput: Write {
    fn sync(&mut self) -> std::io::Result<()>;
}
impl RecordOutput for BufWriter<File> {
    fn sync(&mut self) -> std::io::Result<()> {
        self.flush()?;
        self.get_ref().sync_all()
    }
}

#[cfg(test)]
fn write_records<W: RecordOutput>(
    receiver: mpsc::Receiver<Record>,
    status: &Mutex<RecordingStatus>,
    writer: W,
    limit: u64,
    part_limit: u64,
    open: impl FnMut(u64) -> std::io::Result<W>,
) -> Result<(), String> {
    write_records_mode(receiver, status, writer, limit, part_limit, open, None)
}
fn write_records_mode<W: RecordOutput>(
    receiver: mpsc::Receiver<Record>,
    status: &Mutex<RecordingStatus>,
    mut writer: W,
    limit: u64,
    part_limit: u64,
    mut open: impl FnMut(u64) -> std::io::Result<W>,
    trigger: Option<TriggerConfig>,
) -> Result<(), String> {
    let mut total = 0_u64;
    let mut part_bytes = 0_u64;
    let mut part = 1;
    let mut pending = 0_u64;
    let mut flushed_at = Instant::now();
    let mut ring = std::collections::VecDeque::<Record>::new();
    let mut ring_bytes = 0_usize;
    let mut deadline = None;
    let mut end_elapsed = None;
    let mut finishing = false;
    let mut draining = false;
    let flush = |writer: &mut W, pending: &mut u64| -> Result<(), String> {
        writer.flush().map_err(|e| e.to_string())?;
        status.lock().map_err(|_| "录制状态锁损坏")?.written += *pending;
        *pending = 0;
        Ok(())
    };
    loop {
        if finishing && ring.is_empty() {
            break;
        }
        if deadline.is_some_and(|end| Instant::now() >= end) && !draining && !finishing {
            let mut s = status.lock().map_err(|_| "录制状态锁损坏")?;
            s.active = false;
            for r in receiver.try_iter() {
                if end_elapsed.is_some_and(|end| r.elapsed_us <= end) {
                    ring.push_back(r);
                } else {
                    s.discarded += 1;
                }
            }
            finishing = true;
            draining = true;
            continue;
        }
        let queued = if draining { ring.pop_front() } else { None };
        if draining && queued.is_none() {
            draining = false;
        }
        let record = match queued
            .map(Ok)
            .unwrap_or_else(|| receiver.recv_timeout(std::time::Duration::from_millis(250)))
        {
            Ok(r) => r,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                flush(&mut writer, &mut pending)?;
                flushed_at = Instant::now();
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        if let Some(t) = trigger
            .as_ref()
            .filter(|_| deadline.is_none() && record.protocol != "recording-context")
        {
            let fault = if t.keyword.is_empty() {
                record.direction == "event"
                    && (record.detail.contains("result=Err(")
                        || record.detail.contains("device-fault"))
            } else {
                record
                    .detail
                    .to_lowercase()
                    .contains(&t.keyword.to_lowercase())
            };
            let size = record.bytes.len() + record.detail.len() + record.source.len() + 256;
            while ring.front().is_some_and(|r| {
                record.elapsed_us.saturating_sub(r.elapsed_us) > t.pre_seconds * 1_000_000
                    || ring_bytes + size > 16 * 1024 * 1024
            }) {
                let old = ring.pop_front().unwrap();
                ring_bytes = ring_bytes
                    .saturating_sub(old.bytes.len() + old.detail.len() + old.source.len() + 256);
                status.lock().map_err(|_| "录制状态锁损坏")?.discarded += 1;
            }
            ring_bytes += size;
            let stamp = record.timestamp_us;
            ring.push_back(record);
            if fault {
                deadline = Some(Instant::now() + std::time::Duration::from_secs(t.post_seconds));
                end_elapsed = ring
                    .back()
                    .map(|r| r.elapsed_us + t.post_seconds * 1_000_000);
                draining = true;
                let mut s = status.lock().map_err(|_| "录制状态锁损坏")?;
                s.armed = false;
                s.triggered_at_us = Some(stamp);
            }
            continue;
        }
        if end_elapsed.is_some_and(|end| record.elapsed_us > end) {
            status.lock().map_err(|_| "录制状态锁损坏")?.discarded += 1;
            continue;
        }
        let mut line = serde_json::to_vec(&record).map_err(|e| e.to_string())?;
        line.push(b'\n');
        if line.len() > 1024 * 1024 {
            return Err("单条录制记录超过 1 MiB，录制已停止；该条及后续记录未保存".into());
        }
        if total + line.len() as u64 > limit {
            flush(&mut writer, &mut pending)?;
            writer.sync().map_err(|e| e.to_string())?;
            return Err("达到容量上限，录制已停止；后续通信不再保存".into());
        }
        if part_bytes > 0 && part_bytes + line.len() as u64 > part_limit {
            flush(&mut writer, &mut pending)?;
            writer.sync().map_err(|e| e.to_string())?;
            part += 1;
            part_bytes = 0;
            writer = open(part).map_err(|e| e.to_string())?;
        }
        writer.write_all(&line).map_err(|e| e.to_string())?;
        total += line.len() as u64;
        part_bytes += line.len() as u64;
        pending += 1;
        status.lock().map_err(|_| "录制状态锁损坏")?.bytes = total;
        if flushed_at.elapsed() >= std::time::Duration::from_millis(250) || pending >= 100 {
            flush(&mut writer, &mut pending)?;
            flushed_at = Instant::now();
        }
    }
    status.lock().map_err(|_| "录制状态锁损坏")?.discarded += ring.len() as u64;
    flush(&mut writer, &mut pending)?;
    writer.sync().map_err(|e| e.to_string())
}

fn finish_recording(
    status: &Mutex<RecordingStatus>,
    path: &std::path::Path,
    result: Result<(), String>,
) {
    let mut s = status.lock().unwrap_or_else(|e| e.into_inner());
    s.active = false;
    s.armed = false;
    if let Err(e) = result {
        s.error = Some(e);
    }
    s.unwritten = s
        .accepted
        .saturating_sub(s.written.saturating_add(s.discarded));
    let summary = s.clone();
    drop(s);
    let save = || -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(&summary).map_err(|e| e.to_string())?;
        let mut file = File::create(path.join("summary.tmp")).map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(path.join("summary.tmp"), path.join("summary.json")).map_err(|e| e.to_string())
    };
    if let Err(error) = save() {
        let mut s = status.lock().unwrap_or_else(|e| e.into_inner());
        let previous = s.error.take().unwrap_or_default();
        s.error = Some(format!(
            "{previous} 摘要落盘失败：{error}；不能确认录制完整性"
        ));
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordPage {
    pub records: Vec<Record>,
    pub next_offset: Option<u64>,
    pub warning: Option<String>,
}
fn session_summary(path: &std::path::Path) -> Option<RecordingStatus> {
    fs::read(path.join("summary.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}
pub fn session_warning(path: &std::path::Path) -> Option<String> {
    match session_summary(path) {
        None => Some("会话仍在录制、摘要损坏或异常退出，未完成落盘确认".into()),
        Some(s)
            if s.active
                || s.written > s.accepted.saturating_sub(s.discarded)
                || s.unwritten
                    != s.accepted
                        .saturating_sub(s.written.saturating_add(s.discarded)) =>
        {
            Some("录制摘要计数或结束状态无效，不能确认完整性".into())
        }
        Some(s) if s.error.is_some() || s.dropped > 0 || s.unwritten > 0 => Some(format!(
            "录制存在缺口：队列丢弃={}，未写入={}，错误={}",
            s.dropped,
            s.unwritten,
            s.error.unwrap_or_default()
        )),
        _ => None,
    }
}
pub fn count_warning(path: &std::path::Path, count: u64) -> Option<String> {
    session_summary(path)
        .filter(|s| !s.active && s.written != count)
        .map(|s| {
            format!(
                "实际可读记录 {count} 条与摘要已写 {} 条不符，可能缺失或损坏分卷",
                s.written
            )
        })
}
pub fn session_files(path: &std::path::Path) -> Result<Vec<PathBuf>, String> {
    let metadata_path = PathBuf::from(path).join("session.json");
    let metadata: serde_json::Value =
        serde_json::from_slice(&fs::read(metadata_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if metadata["format"] != "servo-recording" || metadata["version"] != 1 {
        return Err("录制会话格式或版本无效".into());
    }
    let mut files = fs::read_dir(path)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter(|e| {
            e.path().is_file()
                && e.file_name().to_string_lossy().starts_with("records-")
                && e.path().extension().is_some_and(|e| e == "jsonl")
        })
        .map(|e| e.path())
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

/// A single sequential pass, including export. Never restarts from the first volume.
pub fn visit_records(
    path: &str,
    mut visit: impl FnMut(Record) -> Result<(), String>,
) -> Result<Option<String>, String> {
    let folder = std::path::Path::new(path);
    let mut warning = session_warning(folder);
    let mut count = 0;
    for file in session_files(folder)? {
        let mut reader = BufReader::new(File::open(file).map_err(|e| e.to_string())?);
        loop {
            use std::io::Read;
            let mut line = Vec::new();
            let n = reader
                .by_ref()
                .take(1024 * 1024 + 1)
                .read_until(b'\n', &mut line)
                .map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            if n > 1024 * 1024 {
                return Err("录制行超过 1 MiB".into());
            }
            if !line.ends_with(b"\n") {
                warning = Some("未完整落盘的尾行未导出".into());
                break;
            }
            match serde_json::from_slice(&line) {
                Ok(record) => {
                    visit(record)?;
                    count += 1;
                }
                Err(_) => warning = Some("损坏记录已跳过，录制不完整".into()),
            }
        }
    }
    if let Some(message) = count_warning(folder, count) {
        warning = Some(message);
    }
    Ok(warning)
}
#[tauri::command]
pub async fn start_recording(
    limit_mb: u64,
    trigger: Option<TriggerConfig>,
    state: State<'_, Recorder>,
) -> Result<RecordingStatus, String> {
    let recorder = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || recorder.start_configured(limit_mb, trigger))
        .await
        .map_err(|e| e.to_string())?
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "entry point used only by the subprocess crash recovery test"]
    fn crash_writer_child() {
        let root = PathBuf::from(
            std::env::var_os("SERVO_RECORDING_CRASH_TEST_ROOT").expect("subprocess test root"),
        );
        let recorder = Recorder::new(root.clone());
        recorder.start(16).unwrap();
        for _ in 0..400 {
            recorder.emit(
                "crash-test",
                "raw",
                "rx",
                0,
                &[1, 2, 3],
                "persist before forced exit",
            );
        }
        while recorder.status().written < 400 {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        fs::write(root.join("ready"), recorder.status().path).unwrap();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(10));
            recorder.emit("crash-test", "raw", "rx", 0, &[4], "running");
        }
    }
    #[test]
    fn forcibly_terminated_process_recovers_complete_records_without_summary() {
        let root = std::env::temp_dir().join(format!("servo-process-crash-{}", timestamp_us()));
        fs::create_dir_all(&root).unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "recording::tests::crash_writer_child",
                "--nocapture",
            ])
            .env("SERVO_RECORDING_CRASH_TEST_ROOT", &root)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        while !root.join("ready").exists() && Instant::now() < deadline {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let ready = root.join("ready").is_file();
        let _ = child.kill();
        child.wait().unwrap();
        assert!(ready, "writer child did not flush its initial records");
        let session = fs::read_to_string(root.join("ready")).unwrap();
        assert!(!PathBuf::from(&session).join("summary.json").exists());
        let index = crate::recording_index::RecordingIndex::new(root.join("indexes"));
        let page = index.page(&session, 0, &Default::default(), false).unwrap();
        assert!(page.total >= 400);
        assert!(page.warning.is_some());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn trigger_evicts_old_records_keeps_context_and_stops_without_new_traffic() {
        let root = std::env::temp_dir().join(format!("servo-trigger-{}", timestamp_us()));
        let recorder = Recorder::new(root.clone());
        recorder
            .start_configured(
                16,
                Some(TriggerConfig {
                    pre_seconds: 0,
                    post_seconds: 1,
                    keyword: String::new(),
                }),
            )
            .unwrap();
        recorder.emit("port", "raw", "rx", 0, &[1], "before");
        recorder.set_context("profile", serde_json::json!({"name":"changed while armed"}));
        recorder.emit(
            "port",
            "raw",
            "event",
            1,
            &[],
            "FC03 request=[] result=Err(Timeout)",
        );
        recorder.emit("port", "raw", "rx", 2, &[2], "after");
        let limit = Instant::now() + std::time::Duration::from_secs(4);
        while recorder.status().active && Instant::now() < limit {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let status = recorder.stop().unwrap();
        assert!(!status.active);
        assert!(!status.armed);
        assert!(status.triggered_at_us.is_some());
        assert_eq!(status.discarded, 1);
        assert_eq!(status.unwritten, 0);
        assert!(status.error.is_none());
        let mut records = Vec::new();
        visit_records(&status.path, |r| {
            records.push(r);
            Ok(())
        })
        .unwrap();
        assert_eq!(records.len(), 3);
        assert!(records.iter().any(|r| r.protocol == "recording-context"));
        assert!(records.iter().any(|r| r.detail == "after"));
        assert!(!records.iter().any(|r| r.detail == "before"));
        recorder
            .start_configured(
                16,
                Some(TriggerConfig {
                    pre_seconds: 30,
                    post_seconds: 1,
                    keyword: "never".into(),
                }),
            )
            .unwrap();
        recorder.emit("port", "raw", "rx", 0, &[3], "normal");
        let stopped = recorder.stop().unwrap();
        assert_eq!(stopped.discarded, 1);
        assert_eq!(stopped.unwritten, 0);
        assert_eq!(stopped.written, 0);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn persists_unknown_protocol_and_recovers_partial_tail() {
        let root = std::env::temp_dir().join(format!("servo-rec-{}", timestamp_us()));
        let r = Recorder::new(root.clone());
        r.start(16).unwrap();
        r.emit("test", "unknown", "rx", 1, &[0, 255], "partial");
        let status = r.stop().unwrap();
        assert_eq!(status.written, 1);
        assert!(!status.active);
        let index = crate::recording_index::RecordingIndex::new(root.join("indexes"));
        let page = index
            .page(&status.path, 0, &Default::default(), false)
            .unwrap();
        assert_eq!(page.records[0].bytes, [0, 255]);
        let mut file = OpenOptions::new()
            .append(true)
            .open(PathBuf::from(&status.path).join("records-0001.jsonl"))
            .unwrap();
        file.write_all(b"{broken").unwrap();
        assert!(index
            .page(&status.path, 0, &Default::default(), false)
            .unwrap()
            .warning
            .is_some());
        fs::remove_dir_all(root).unwrap();
    }
    fn test_record() -> Record {
        Record {
            sequence: 1,
            timestamp_us: 1,
            elapsed_us: 1,
            source: "test".into(),
            protocol: "unknown".into(),
            direction: "rx".into(),
            transaction: 1,
            bytes: vec![0; 16],
            detail: String::new(),
        }
    }
    struct FailingOutput {
        mode: &'static str,
    }
    impl Write for FailingOutput {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.mode == "write" {
                Err(std::io::Error::other("injected disk full"))
            } else {
                Ok(bytes.len())
            }
        }
        fn flush(&mut self) -> std::io::Result<()> {
            if self.mode == "flush" {
                Err(std::io::Error::other("injected flush failure"))
            } else {
                Ok(())
            }
        }
    }
    impl RecordOutput for FailingOutput {
        fn sync(&mut self) -> std::io::Result<()> {
            if self.mode == "sync" {
                Err(std::io::Error::other("injected sync failure"))
            } else {
                Ok(())
            }
        }
    }
    #[test]
    fn disk_write_flush_and_sync_failures_remain_failures() {
        for mode in ["write", "flush", "sync"] {
            let (tx, rx) = mpsc::sync_channel(2);
            tx.send(test_record()).unwrap();
            drop(tx);
            let status = Mutex::new(RecordingStatus {
                active: true,
                accepted: 1,
                ..Default::default()
            });
            let result = write_records(
                rx,
                &status,
                FailingOutput { mode },
                10000,
                10000,
                |_| unreachable!(),
            );
            assert!(result.unwrap_err().contains("injected"));
            if mode != "sync" {
                assert_eq!(status.lock().unwrap().written, 0);
            }
        }
    }
    #[test]
    fn capacity_rotation_and_summary_failure_are_reported() {
        let root = std::env::temp_dir().join(format!("servo-fault-{}", timestamp_us()));
        let recorder = Recorder::new(root.clone());
        let status = recorder.start_with_limits(1, 1).unwrap();
        recorder.emit("test", "raw", "rx", 1, &[1], "");
        let end = recorder.stop().unwrap();
        assert!(end.error.unwrap().contains("容量"));
        assert_eq!(end.written, 0);
        assert_eq!(end.unwritten, 1);
        let status2 = recorder.start_with_limits(10000, 1).unwrap();
        fs::create_dir(PathBuf::from(&status2.path).join("records-0002.jsonl")).unwrap();
        recorder.emit("test", "raw", "rx", 1, &[1], "");
        recorder.emit("test", "raw", "rx", 2, &[2], "");
        let end = recorder.stop().unwrap();
        assert!(end.error.is_some());
        assert_eq!(end.written, 1);
        assert_eq!(end.unwritten, 1);
        let index = crate::recording_index::RecordingIndex::new(root.join("indexes"));
        let page = index
            .page(&status2.path, 0, &Default::default(), false)
            .unwrap();
        assert_eq!(page.total, 1);
        assert!(page.warning.is_some());
        let status3 = recorder.start(16).unwrap();
        fs::create_dir(PathBuf::from(&status3.path).join("summary.json")).unwrap();
        recorder.emit("test", "raw", "rx", 1, &[1], "");
        let end = recorder.stop().unwrap();
        assert!(end.error.unwrap().contains("摘要落盘失败"));
        assert!(session_warning(std::path::Path::new(&status.path)).is_some());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bounded_queue_drops_new_records_and_handles_disconnected_worker() {
        let recorder = Recorder::new(PathBuf::new());
        let (sender, receiver) = mpsc::sync_channel(1);
        *recorder.run.lock().unwrap() = Some(Run {
            sender,
            worker: std::thread::spawn(|| {}),
            started: Instant::now(),
        });
        recorder.status.lock().unwrap().active = true;
        recorder.emit("test", "raw", "rx", 1, &[1], "");
        recorder.emit("test", "raw", "rx", 2, &[2], "");
        assert_eq!(recorder.status().accepted, 1);
        assert_eq!(recorder.status().dropped, 1);
        assert_eq!(receiver.recv().unwrap().bytes, [1]);
        drop(receiver);
        recorder.emit("test", "raw", "rx", 3, &[3], "");
        assert!(!recorder.status().active);
        assert_eq!(recorder.status().dropped, 2);
        recorder.stop().unwrap();
    }
    #[test]
    fn stop_drains_queue_and_repeated_stop_is_idempotent() {
        let root = std::env::temp_dir().join(format!("servo-drain-{}", timestamp_us()));
        let recorder = Recorder::new(root.clone());
        recorder.start(16).unwrap();
        for n in 0..500 {
            recorder.emit("test", "raw", "rx", n, &[1, 2, 3], "");
        }
        let end = recorder.stop().unwrap();
        assert_eq!(end.written, end.accepted);
        assert_eq!(end.unwritten, 0);
        assert!(end.error.is_none());
        assert_eq!(recorder.stop().unwrap().written, end.written);
        let mut count = 0;
        assert!(visit_records(&end.path, |_| {
            count += 1;
            Ok(())
        })
        .unwrap()
        .is_none());
        assert_eq!(count, end.written);
        fs::remove_dir_all(root).unwrap();
    }
}
