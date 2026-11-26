//! 运行时状态与 IPC 命令门面：状态、会话与错误留在本文件，命令按职责拆到子模块。
//!
//! 契约：`#[tauri::command]` 的函数名、参数名与返回结构字段名不可改；`lib.rs` 的
//! `generate_handler!` 依赖这里 re-export 的命令名。
use crate::audit::{AuditEntry, AuditStore, AuditStoreError};
use crate::discovery::DiscoveryControl;
use crate::modbus::{CommunicationSettings, ModbusError, SerialClient};
use crate::modbus_tcp::TcpClient;
use crate::profile::{ProfileError, ServoProfile};
use crate::scan::ScanControl;
use self::simulator::SimulatorDevice;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tauri::State;
use thiserror::Error;

mod comm;
mod connect;
mod discovery_cmd;
mod operations;
mod params;
mod scan_cmd;
mod simulator;
mod snapshot;
mod types;

#[cfg(test)]
mod tests;

pub use comm::{configure_communication, get_communication_stats, probe_read};
pub use connect::{
    ble_scan, connect_device, disconnect_device, get_active_profile, get_connection_status,
    import_profile, list_serial_ports,
};
pub use discovery_cmd::{cancel_discovery, discover_device, get_discovery_status};
pub use operations::{apply_parameters, persist_parameters};
pub use params::{batch_write_parameters, read_parameters, read_statuses, write_parameter};
pub use scan_cmd::{cancel_register_scan, register_scan_status, scan_registers};
pub use snapshot::{capture_parameter_snapshot, compare_parameter_snapshot};
pub use types::*;

#[derive(Debug, Error)]
enum RuntimeError {
    #[error("{0}")]
    Profile(#[from] ProfileError),
    #[error("{0}")]
    Modbus(#[from] ModbusError),
    #[error("串口错误：{0}")]
    Serial(#[from] serialport::Error),
    #[error("{0}")]
    Audit(#[from] AuditStoreError),
    #[error("参数快照无效：{0}")]
    Snapshot(String),
    #[error("尚未导入设备配置")]
    NoProfile,
    #[error("尚未连接设备")]
    NotConnected,
    #[error("设备已经连接，请先断开")]
    AlreadyConnected,
    #[error("参数不存在：{0}")]
    UnknownParameter(String),
    #[error("参数 {0} 为只读")]
    ReadOnly(String),
    #[error("安全确认不完整：{0}")]
    Confirmation(String),
    #[error("参数 {parameter_id} 已被其他操作修改：期望原始值 {expected}，实际为 {actual}")]
    StaleValue {
        parameter_id: String,
        expected: u16,
        actual: u16,
    },
    #[error("参数 {parameter_id} 写入后回读不一致：写入 {written}，回读 {read_back}")]
    ReadBackMismatch {
        parameter_id: String,
        written: u16,
        read_back: u16,
    },
    #[error("操作 {0} 超时，未得到设备成功状态")]
    OperationTimeout(String),
    #[error("当前配置未定义{0}操作")]
    UnsupportedOperation(String),
    #[error("内部状态锁已损坏")]
    Poisoned,
    #[error("后台任务失败：{0}")]
    Task(String),
}

pub struct AppState {
    inner: Arc<Mutex<Runtime>>,
    discovery: Arc<DiscoveryControl>,
    scan: Arc<ScanControl>,
}

impl AppState {
    pub fn with_recorder(
        database_path: PathBuf,
        recorder: crate::recording::Recorder,
    ) -> Result<Self, AuditStoreError> {
        recorder.set_context("profile", serde_json::Value::Null);
        recorder.set_context("connection", serde_json::Value::Null);
        recorder.set_context(
            "communication",
            serde_json::json!(CommunicationSettings::default()),
        );
        Ok(Self {
            discovery: Arc::default(),
            scan: Arc::default(),
            inner: Arc::new(Mutex::new(Runtime {
                profile: None,
                session: None,
                audit_store: AuditStore::open(database_path)?,
                communication: CommunicationSettings::default(),
                recorder,
            })),
        })
    }
}

struct Runtime {
    profile: Option<ServoProfile>,
    session: Option<Session>,
    audit_store: AuditStore,
    communication: CommunicationSettings,
    recorder: crate::recording::Recorder,
}

enum Session {
    Simulator(SimulatorDevice),
    Serial(SerialClient),
    Ble(SerialClient),
    Tcp(TcpClient),
}

impl Session {
    fn read_registers(&mut self, address: u16, count: u16) -> Result<Vec<u16>, RuntimeError> {
        match self {
            Session::Simulator(device) => Ok(device.read_registers(address, count)),
            Session::Serial(client) => Ok(client.read_holding_registers(address, count)?),
            Session::Ble(client) => Ok(client.read_holding_registers(address, count)?),
            Session::Tcp(client) => Ok(client.read_holding_registers(address, count)?),
        }
    }

    fn write_register(&mut self, address: u16, value: u16) -> Result<(), RuntimeError> {
        match self {
            Session::Simulator(device) => {
                device.write_register(address, value);
                Ok(())
            }
            Session::Serial(client) => Ok(client.write_single_register(address, value)?),
            Session::Ble(client) => Ok(client.write_single_register(address, value)?),
            Session::Tcp(client) => Ok(client.write_single_register(address, value)?),
        }
    }

    fn mode(&self) -> ConnectionMode {
        match self {
            Session::Simulator(_) => ConnectionMode::Simulator,
            Session::Serial(_) => ConnectionMode::Serial,
            Session::Ble(_) => ConnectionMode::Ble,
            Session::Tcp(_) => ConnectionMode::Tcp,
        }
    }
}

async fn with_runtime<T, F>(state: State<'_, AppState>, operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&mut Runtime) -> Result<T, RuntimeError> + Send + 'static,
{
    if state.discovery.running.load(Ordering::SeqCst) {
        return Err("自动查找占用串口，请先取消或等待完成".into());
    }
    if state.scan.running.load(Ordering::SeqCst) {
        return Err("寄存器扫描占用串口，请先取消或等待完成".into());
    }
    let inner = state.inner.clone();
    let discovery = state.discovery.clone();
    let scan = state.scan.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut runtime = inner.lock().map_err(|_| RuntimeError::Poisoned)?;
        if discovery.running.load(Ordering::SeqCst) {
            return Err(RuntimeError::Task("自动查找正在运行".into()));
        }
        if scan.running.load(Ordering::SeqCst) {
            return Err(RuntimeError::Task("寄存器扫描正在运行".into()));
        }
        let result = operation(&mut runtime);
        let events = match runtime.session.as_mut() {
            Some(Session::Serial(client)) => std::mem::take(&mut client.events),
            Some(Session::Ble(client)) => std::mem::take(&mut client.events),
            Some(Session::Tcp(client)) => std::mem::take(&mut client.events),
            _ => Vec::new(),
        };
        for (status, detail) in events {
            // A logging failure must not change a completed device operation into a retryable failure.
            if let Err(error) = runtime
                .audit_store
                .append("communication.read", &status, &detail)
            {
                eprintln!("通讯审计写入失败：{error}");
            }
        }
        result
    })
    .await
    .map_err(|error| RuntimeError::Task(error.to_string()).to_string())?
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_audit_log(state: State<'_, AppState>) -> Result<Vec<AuditEntry>, String> {
    with_runtime(state, |runtime| Ok(runtime.audit_store.list(500)?)).await
}

fn connection_status(runtime: &Runtime) -> ConnectionStatus {
    ConnectionStatus {
        connected: runtime.session.is_some(),
        mode: runtime.session.as_ref().map(Session::mode),
        device_name: runtime.profile.as_ref().map(|p| p.device.name.clone()),
    }
}

impl Runtime {
    fn push_audit(
        &mut self,
        action: &str,
        status: &str,
        detail: &str,
    ) -> Result<AuditEntry, RuntimeError> {
        Ok(self.audit_store.append(action, status, detail)?)
    }
}
