#[cfg(windows)]
use crate::pcap_file::PcapWriter;
use crate::recording::Recorder;
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::JoinHandle,
};
use tauri::State;
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStatus {
    pub active: bool,
    pub path: String,
    pub packets: u64,
    pub bytes: u64,
    pub driver_dropped: Option<u32>,
    pub error: Option<String>,
}
struct Run {
    stop: Arc<AtomicBool>,
    worker: JoinHandle<()>,
}
#[derive(Clone)]
pub struct NetworkCapture {
    run: Arc<Mutex<Option<Run>>>,
    status: Arc<Mutex<CaptureStatus>>,
    root: PathBuf,
    recorder: Recorder,
}
impl NetworkCapture {
    pub fn new(root: PathBuf, recorder: Recorder) -> Self {
        Self {
            run: Arc::default(),
            status: Arc::default(),
            root,
            recorder,
        }
    }
    pub fn status(&self) -> CaptureStatus {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn stop(&self) -> Result<CaptureStatus, String> {
        let mut slot = self.run.lock().map_err(|_| "捕获锁损坏")?;
        if let Some(r) = slot.take() {
            r.stop.store(true, Ordering::Relaxed);
            r.worker.join().map_err(|_| "捕获线程异常退出")?;
        }
        Ok(self.status())
    }
    fn start(
        &self,
        adapter: String,
        ethercat_only: bool,
        limit_mb: u64,
    ) -> Result<CaptureStatus, String> {
        if !(16..=16384).contains(&limit_mb) {
            return Err("容量须为 16～16384 MiB".into());
        }
        let mut slot = self.run.lock().map_err(|_| "捕获锁损坏")?;
        if slot.is_some() {
            return Err("请先停止上一捕获任务".into());
        }
        if !crate::master::adapters()?.iter().any(|a| a.name == adapter) {
            return Err("网卡不存在".into());
        }
        std::fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let directory = self
            .root
            .join(format!("capture-{}", crate::recording::timestamp_us()));
        std::fs::create_dir(&directory).map_err(|e| e.to_string())?;
        *self.status.lock().map_err(|_| "捕获状态锁损坏")? = CaptureStatus {
            active: true,
            path: directory.to_string_lossy().into(),
            ..Default::default()
        };
        let status = self.status.clone();
        let recorder = self.recorder.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || {
            let result = run_capture(
                &adapter,
                ethercat_only,
                limit_mb,
                &directory,
                &recorder,
                &signal,
                &status,
                ready_tx,
            );
            let mut s = status.lock().unwrap_or_else(|e| e.into_inner());
            s.active = false;
            if let Err(e) = result {
                s.error = Some(e);
            }
            if let Ok(bytes) = serde_json::to_vec_pretty(&*s) {
                let _ = std::fs::write(directory.join("summary.json"), bytes);
            }
        });
        *slot = Some(Run { stop, worker });
        ready_rx.recv().map_err(|_| "捕获线程无法启动")??;
        Ok(self.status())
    }
}
#[allow(clippy::too_many_arguments)]
fn run_capture(
    adapter: &str,
    only: bool,
    limit_mb: u64,
    directory: &std::path::Path,
    recorder: &Recorder,
    stop: &AtomicBool,
    status: &Mutex<CaptureStatus>,
    ready: std::sync::mpsc::SyncSender<Result<(), String>>,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        use crate::master::native;
        let name = std::ffi::CString::new(adapter).map_err(|_| "网卡名称无效")?;
        let mut error = [0u8; 256];
        let handle = unsafe { native::sa_capture_open(name.as_ptr(), error.as_mut_ptr().cast()) };
        if handle.is_null() {
            let e = String::from_utf8_lossy(&error)
                .trim_end_matches('\0')
                .to_string();
            let _ = ready.send(Err(e.clone()));
            return Err(e);
        }
        struct Handle(*mut std::ffi::c_void);
        impl Drop for Handle {
            fn drop(&mut self) {
                unsafe { native::sa_capture_close(self.0) }
            }
        }
        let handle = Handle(handle);
        let mut part = 1;
        let mut part_bytes = 0u64;
        let mut writer = PcapWriter::create(&directory.join("frames-0001.pcapng"))?;
        let _ = ready.send(Ok(()));
        let mut buffer = vec![0u8; 65536];
        let mut last = std::time::Instant::now();
        while !stop.load(Ordering::Relaxed) {
            let mut stamp = 0;
            let mut original = 0;
            let n = unsafe {
                native::sa_capture_next(
                    handle.0,
                    buffer.as_mut_ptr(),
                    buffer.len() as u32,
                    &mut stamp,
                    &mut original,
                )
            };
            if n < 0 {
                return Err("抓包驱动读取失败".into());
            }
            if n == 0 {
                std::thread::sleep(std::time::Duration::from_millis(2));
            } else if !only || is_ethercat(&buffer[..n as usize]) {
                let data = &buffer[..n as usize];
                let total = status.lock().map_err(|_| "捕获状态锁损坏")?.bytes;
                if total + n as u64 + 32 > limit_mb * 1024 * 1024 {
                    writer.finish()?;
                    return Err("达到容量上限，在线捕获已停止".into());
                }
                if part_bytes > 16 * 1024 * 1024 {
                    writer.finish()?;
                    part += 1;
                    part_bytes = 0;
                    writer =
                        PcapWriter::create(&directory.join(format!("frames-{part:04}.pcapng")))?;
                }
                writer.packet(adapter, data, stamp, original)?;
                recorder.emit_at(
                    &format!("capture:{adapter}"),
                    "ethernet",
                    "unknown",
                    0,
                    data,
                    &format!("passive capture originalLength={original}"),
                    stamp,
                );
                part_bytes += n as u64 + 32;
                let mut s = status.lock().map_err(|_| "捕获状态锁损坏")?;
                s.packets += 1;
                s.bytes += n as u64 + 32;
            }
            if last.elapsed().as_millis() >= 250 {
                writer.flush()?;
                let drops = unsafe { native::sa_capture_drops(handle.0) };
                status.lock().map_err(|_| "捕获状态锁损坏")?.driver_dropped =
                    u32::try_from(drops).ok();
                last = std::time::Instant::now();
            }
        }
        let drops = unsafe { native::sa_capture_drops(handle.0) };
        status.lock().map_err(|_| "捕获状态锁损坏")?.driver_dropped = u32::try_from(drops).ok();
        writer.finish()
    }
    #[cfg(not(windows))]
    {
        let _ = (adapter, only, limit_mb, directory, recorder, stop, status);
        let _ = ready.send(Err("在线捕获仅支持 Windows/Npcap".into()));
        Err("在线捕获仅支持 Windows/Npcap".into())
    }
}
#[cfg(any(windows, test))]
fn is_ethercat(data: &[u8]) -> bool {
    if data.len() < 14 {
        return false;
    }
    let mut p = 12;
    loop {
        if data.len() < p + 2 {
            return false;
        }
        match u16::from_be_bytes([data[p], data[p + 1]]) {
            0x88a4 => return true,
            0x8100 | 0x88a8 => p += 4,
            _ => return false,
        }
    }
}
#[tauri::command]
pub async fn start_network_capture(
    adapter: String,
    ethercat_only: bool,
    limit_mb: u64,
    state: State<'_, NetworkCapture>,
) -> Result<CaptureStatus, String> {
    let c = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || c.start(adapter, ethercat_only, limit_mb))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn stop_network_capture(
    state: State<'_, NetworkCapture>,
) -> Result<CaptureStatus, String> {
    let c = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || c.stop())
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn network_capture_status(state: State<'_, NetworkCapture>) -> CaptureStatus {
    state.status()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vlan_filter() {
        assert!(is_ethercat(&[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x81, 0, 0, 1, 0x88, 0xa4
        ]));
        assert!(!is_ethercat(&[0; 14]));
    }
}
