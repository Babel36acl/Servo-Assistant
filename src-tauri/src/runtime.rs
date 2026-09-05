use crate::audit::{now_ms, AuditEntry, AuditStore, AuditStoreError};
use crate::discovery::{self, DiscoveryControl, DiscoveryStatus};
use crate::modbus::{
    CommunicationSettings, CommunicationStats, ModbusError, Protocol, SerialClient,
};
use crate::profile::{
    decode_value, encode_value, Access, OperationDefinition, OperationSet, ParameterDefinition,
    ParitySetting, ProfileError, RiskLevel, ServoProfile, StatusDefinition,
};
use serde::{Deserialize, Serialize};
use serialport::{DataBits, Parity, StopBits};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::State;
use thiserror::Error;

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
}

impl Session {
    fn read_registers(&mut self, address: u16, count: u16) -> Result<Vec<u16>, RuntimeError> {
        match self {
            Session::Simulator(device) => Ok(device.read_registers(address, count)),
            Session::Serial(client) => Ok(client.read_holding_registers(address, count)?),
        }
    }

    fn write_register(&mut self, address: u16, value: u16) -> Result<(), RuntimeError> {
        match self {
            Session::Simulator(device) => {
                device.write_register(address, value);
                Ok(())
            }
            Session::Serial(client) => Ok(client.write_single_register(address, value)?),
        }
    }

    fn mode(&self) -> ConnectionMode {
        match self {
            Session::Simulator(_) => ConnectionMode::Simulator,
            Session::Serial(_) => ConnectionMode::Serial,
        }
    }
}

struct SimulatorDevice {
    registers: HashMap<u16, u16>,
    eeprom: HashMap<u16, u16>,
    parameter_addresses: HashSet<u16>,
    operations: Option<OperationSet>,
    recorder: Option<crate::recording::Recorder>,
}

impl SimulatorDevice {
    fn new(profile: &ServoProfile) -> Result<Self, RuntimeError> {
        let mut registers = HashMap::new();
        for parameter in &profile.parameters {
            registers.insert(
                parameter.address,
                encode_value(parameter, parameter.default_value)?,
            );
        }
        for status in &profile.statuses {
            registers.insert(status.address, 0);
        }
        if let Some(operations) = &profile.operations {
            registers.insert(operations.command_register, 0);
            registers.insert(operations.status_register, 0);
        }
        let parameter_addresses = profile
            .parameters
            .iter()
            .map(|parameter| parameter.address)
            .collect::<HashSet<_>>();
        Ok(Self {
            eeprom: registers
                .iter()
                .filter(|(address, _)| parameter_addresses.contains(address))
                .map(|(address, value)| (*address, *value))
                .collect(),
            parameter_addresses,
            registers,
            operations: profile.operations.clone(),
            recorder: None,
        })
    }

    fn read_registers(&self, address: u16, count: u16) -> Vec<u16> {
        if let Some(r) = &self.recorder {
            r.emit(
                "simulator",
                "simulator",
                "event",
                0,
                &[],
                &format!(
                    "read address={address} count={count} values={:?}",
                    (0..count)
                        .map(|i| self
                            .registers
                            .get(&address.wrapping_add(i))
                            .copied()
                            .unwrap_or(0))
                        .collect::<Vec<_>>()
                ),
            );
        }
        (0..count)
            .map(|offset| {
                *self
                    .registers
                    .get(&address.wrapping_add(offset))
                    .unwrap_or(&0)
            })
            .collect()
    }

    fn write_register(&mut self, address: u16, value: u16) {
        if let Some(r) = &self.recorder {
            r.emit(
                "simulator",
                "simulator",
                "event",
                0,
                &value.to_le_bytes(),
                &format!("write address={address} value={value}"),
            );
        }
        self.registers.insert(address, value);
        let Some(operations) = &self.operations else {
            return;
        };
        if address != operations.command_register {
            return;
        }
        if let Some(operation) = operations
            .apply
            .as_ref()
            .filter(|operation| value == operation.command)
        {
            self.registers
                .insert(operations.status_register, operation.success_status);
        } else if let Some(operation) = operations
            .persist
            .as_ref()
            .filter(|operation| value == operation.command)
        {
            self.eeprom = self
                .registers
                .iter()
                .filter(|(register, _)| self.parameter_addresses.contains(register))
                .map(|(register, current)| (*register, *current))
                .collect();
            self.registers
                .insert(operations.status_register, operation.success_status);
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionMode {
    Simulator,
    Serial,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionPreset {
    #[default]
    Profile,
    P300,
}
impl ConnectionRequest {
    fn rates(&self, profile: &ServoProfile) -> Vec<u32> {
        if self.preset == ConnectionPreset::P300 {
            vec![4800, 9600, 19200, 38400, 57600, 115200]
        } else {
            profile.transport.allowed_baud_rates.clone()
        }
    }
    fn validate(&self, profile: &ServoProfile) -> Result<(), String> {
        let max = if self.preset == ConnectionPreset::P300 {
            32
        } else {
            247
        };
        if !(1..=max).contains(&self.slave_id) {
            return Err(format!("站号必须为 1..{max}"));
        }
        if !self.rates(profile).contains(&self.baud_rate) || self.baud_rate == 0 {
            return Err("波特率不属于当前通讯预设".into());
        }
        if !matches!(self.stop_bits, 1 | 2)
            || (self.preset == ConnectionPreset::P300 && self.stop_bits != 1)
        {
            return Err("当前通讯预设不支持该停止位".into());
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRequest {
    #[serde(default)]
    pub protocol: Protocol,
    #[serde(default)]
    pub preset: ConnectionPreset,
    pub mode: ConnectionMode,
    pub port_name: Option<String>,
    pub slave_id: u8,
    pub baud_rate: u32,
    pub parity: ParitySetting,
    pub stop_bits: u8,
    pub timeout_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStatus {
    pub connected: bool,
    pub mode: Option<ConnectionMode>,
    pub device_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub device_id: String,
    pub device_name: String,
    pub profile_version: String,
    pub parameter_count: usize,
    pub status_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerialPortInfo {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterValue {
    pub parameter_id: String,
    pub raw: u16,
    pub value: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteRequest {
    pub parameter_id: String,
    pub value: f64,
    pub expected_raw: Option<u16>,
    pub confirmed: bool,
    pub confirmation_phrase: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteResult {
    pub parameter_id: String,
    pub previous_raw: u16,
    pub written_raw: u16,
    pub read_back_raw: u16,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotValue {
    pub parameter_id: String,
    pub raw: u16,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterSnapshot {
    pub schema_version: String,
    pub created_at_ms: u64,
    pub label: String,
    pub device_id: String,
    pub device_name: String,
    pub profile_version: String,
    pub values: Vec<SnapshotValue>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDiff {
    pub parameter_id: String,
    pub name: String,
    pub current_raw: u16,
    pub current_value: f64,
    pub target_raw: u16,
    pub target_value: f64,
    pub changed: bool,
    pub writable: bool,
    pub risk: RiskLevel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchWriteItem {
    pub parameter_id: String,
    pub value: f64,
    pub expected_raw: u16,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchWriteRequest {
    pub items: Vec<BatchWriteItem>,
    pub confirmation_phrase: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchWriteResult {
    pub completed: Vec<WriteResult>,
    pub failed_parameter_id: Option<String>,
    pub error: Option<String>,
}

type PlannedWrite = (ParameterDefinition, u16, u16);

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusValue {
    pub id: String,
    pub name: String,
    pub address: u16,
    pub raw: u16,
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationResult {
    pub operation: String,
    pub command: u16,
    pub observed_status: u16,
}

async fn with_runtime<T, F>(state: State<'_, AppState>, operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&mut Runtime) -> Result<T, RuntimeError> + Send + 'static,
{
    if state.discovery.running.load(Ordering::SeqCst) {
        return Err("自动查找占用串口，请先取消或等待完成".into());
    }
    let inner = state.inner.clone();
    let discovery = state.discovery.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut runtime = inner.lock().map_err(|_| RuntimeError::Poisoned)?;
        if discovery.running.load(Ordering::SeqCst) {
            return Err(RuntimeError::Task("自动查找正在运行".into()));
        }
        let result = operation(&mut runtime);
        let events = match runtime.session.as_mut() {
            Some(Session::Serial(client)) => std::mem::take(&mut client.events),
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryRequest {
    connection: ConnectionRequest,
    start_slave: u8,
    end_slave: u8,
}

#[tauri::command]
pub fn cancel_discovery(state: State<'_, AppState>) {
    state.discovery.cancel.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub fn get_discovery_status(state: State<'_, AppState>) -> Result<DiscoveryStatus, String> {
    state
        .discovery
        .status
        .lock()
        .map(|status| status.clone())
        .map_err(|_| "探测状态锁已损坏".into())
}

#[tauri::command]
pub async fn discover_device(
    request: DiscoveryRequest,
    state: State<'_, AppState>,
) -> Result<DiscoveryStatus, String> {
    if state
        .discovery
        .running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("自动查找已经运行".into());
    }
    state.discovery.cancel.store(false, Ordering::SeqCst);
    let worker_control = state.discovery.clone();
    let inner = state.inner.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<DiscoveryStatus, String> {
        let _lease = discovery::DiscoveryLease(worker_control.clone());
        let mut runtime = inner.lock().map_err(|_| "内部状态锁已损坏")?;
        if runtime.session.is_some() { return Err("请先断开连接再自动查找".into()); }
        let profile = runtime.profile.as_ref().ok_or("请先导入设备 Profile")?;
        let mut connection = request.connection;
        if !matches!(connection.mode, ConnectionMode::Serial) { return Err("自动查找仅适用于真实串口".into()); }
        connection.validate(profile)?;
        if connection.preset == ConnectionPreset::P300 && request.end_slave > 32 { return Err("P300 站号范围为 1..32".into()); }
        let pairs = discovery::candidates(request.start_slave, request.end_slave, connection.slave_id, connection.baud_rate, &connection.rates(profile))?;
        let preferred = discovery::SerialMode { protocol: connection.protocol, parity: connection.parity, stop_bits: connection.stop_bits };
        let candidates = discovery::modes(preferred).into_iter().flat_map(|mode| pairs.iter().map(move |&(baud, slave)| (baud, slave, mode))).collect::<Vec<_>>();
        let address = profile.statuses.first().map(|item| item.address)
            .or_else(|| profile.parameters.first().map(|item| item.address)).ok_or("Profile 没有可读取的地址")?;
        let mut client: Option<(u32, discovery::SerialMode, SerialClient)> = None;
        let result = discovery::scan(&worker_control, &candidates, |baud, slave, mode| {
            if client.as_ref().is_none_or(|(rate, current, _)| *rate != baud || *current != mode) {
                // Close the previous baud-rate handle before reopening the same port.
                client = None;
                connection.protocol = mode.protocol;
                connection.parity = mode.parity;
                connection.stop_bits = mode.stop_bits;
                connection.baud_rate = baud;
                connection.slave_id = slave;
                client = Some((baud, mode, open_serial_client(&connection, &runtime.recorder).map_err(|error| error.to_string())?));
            }
            client.as_mut().unwrap().2.detect_slave(slave, address).map_err(|error| error.to_string())
        });
        drop(client);
        let detail = match &result {
            Ok(status) => format!("port={:?} checked={}/{} found={} cancelled={} slave={:?} baud={:?} mode={:?} address=0x{address:04X}", connection.port_name, status.completed, status.total, status.found, status.cancelled, status.slave_id, status.baud_rate, status.serial_mode),
            Err(error) => error.clone(),
        };
        runtime.push_audit("communication.discovery", if result.is_ok() { "completed" } else { "failed" }, &detail).map_err(|error| error.to_string())?;
        result
    }).await.map_err(|error| error.to_string()).and_then(|result| result);
    result
}

#[tauri::command]
pub async fn import_profile(
    profile_json: String,
    state: State<'_, AppState>,
) -> Result<ProfileSummary, String> {
    with_runtime(state, move |runtime| {
        if runtime.session.is_some() {
            return Err(RuntimeError::AlreadyConnected);
        }
        let profile = ServoProfile::from_json(&profile_json)?;
        let summary = profile_summary(&profile);
        runtime.profile = Some(profile);
        runtime
            .recorder
            .set_context("profile", serde_json::json!(runtime.profile));
        runtime.push_audit("profile.import", "success", &summary.device_name)?;
        Ok(summary)
    })
    .await
}

#[tauri::command]
pub async fn get_active_profile(
    state: State<'_, AppState>,
) -> Result<Option<ServoProfile>, String> {
    with_runtime(state, |runtime| Ok(runtime.profile.clone())).await
}

#[tauri::command]
pub async fn list_serial_ports() -> Result<Vec<SerialPortInfo>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        serialport::available_ports()
            .map(|ports| {
                ports
                    .into_iter()
                    .map(|port| SerialPortInfo {
                        name: port.port_name,
                        description: format!("{:?}", port.port_type),
                    })
                    .collect()
            })
            .map_err(|error| RuntimeError::Serial(error).to_string())
    })
    .await
    .map_err(|error| RuntimeError::Task(error.to_string()).to_string())?
}

fn open_serial_client(
    request: &ConnectionRequest,
    recorder: &crate::recording::Recorder,
) -> Result<SerialClient, RuntimeError> {
    let port_name = request
        .port_name
        .as_deref()
        .ok_or_else(|| RuntimeError::Profile(ProfileError::Validation("必须选择串口".into())))?;
    let parity = match request.parity {
        ParitySetting::None => Parity::None,
        ParitySetting::Even => Parity::Even,
        ParitySetting::Odd => Parity::Odd,
    };
    let stop_bits = match request.stop_bits {
        1 => StopBits::One,
        2 => StopBits::Two,
        _ => {
            return Err(RuntimeError::Profile(ProfileError::Validation(
                "停止位只能为 1 或 2".into(),
            )))
        }
    };
    let port = serialport::new(port_name, request.baud_rate)
        .data_bits(DataBits::Eight)
        .parity(parity)
        .stop_bits(stop_bits)
        .timeout(Duration::from_millis(request.timeout_ms.clamp(100, 10_000)))
        .open()
        .map_err(|error| {
            RuntimeError::Task(format!(
                "无法打开 {port_name}：{error}。若拒绝访问，请检查端口是否被其他程序或测试占用。"
            ))
        })?;
    let bits_per_char = 1
        + 8
        + u32::from(!matches!(request.parity, ParitySetting::None))
        + request.stop_bits as u32;
    Ok(
        SerialClient::new(port, request.slave_id, request.baud_rate, bits_per_char)
            .with_protocol(request.protocol)
            .with_recorder(recorder.clone()),
    )
}

#[tauri::command]
pub async fn connect_device(
    request: ConnectionRequest,
    state: State<'_, AppState>,
) -> Result<ConnectionStatus, String> {
    with_runtime(state, move |runtime| {
        if runtime.session.is_some() {
            return Err(RuntimeError::AlreadyConnected);
        }
        let profile = runtime.profile.clone().ok_or(RuntimeError::NoProfile)?;
        request
            .validate(&profile)
            .map_err(|error| RuntimeError::Profile(ProfileError::Validation(error)))?;
        let session = match request.mode {
            ConnectionMode::Simulator => Session::Simulator(SimulatorDevice::new(&profile)?),
            ConnectionMode::Serial => {
                Session::Serial(open_serial_client(&request, &runtime.recorder)?)
            }
        };
        let mode = session.mode();
        runtime
            .recorder
            .set_context("connection", serde_json::json!(request));
        runtime.session = Some(session);
        if let Some(Session::Simulator(device)) = runtime.session.as_mut() {
            device.recorder = Some(runtime.recorder.clone());
        }
        if let Some(Session::Serial(client)) = runtime.session.as_mut() {
            client.settings = runtime.communication.clone();
        }
        runtime.push_audit(
            "connection.open",
            "success",
            &format!("mode={mode:?}, slave={}", request.slave_id),
        )?;
        Ok(ConnectionStatus {
            connected: true,
            mode: Some(mode),
            device_name: Some(profile.device.name),
        })
    })
    .await
}

#[tauri::command]
pub async fn disconnect_device(state: State<'_, AppState>) -> Result<ConnectionStatus, String> {
    with_runtime(state, |runtime| {
        runtime.session = None;
        runtime
            .recorder
            .set_context("connection", serde_json::Value::Null);
        runtime.push_audit("connection.close", "success", "用户主动断开")?;
        Ok(connection_status(runtime))
    })
    .await
}

#[tauri::command]
pub async fn get_connection_status(state: State<'_, AppState>) -> Result<ConnectionStatus, String> {
    with_runtime(state, |runtime| Ok(connection_status(runtime))).await
}

#[tauri::command]
pub async fn read_parameters(
    parameter_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ParameterValue>, String> {
    with_runtime(state, move |runtime| {
        let profile = runtime.profile.clone().ok_or(RuntimeError::NoProfile)?;
        let definitions = if parameter_ids.is_empty() {
            profile.parameters.clone()
        } else {
            parameter_ids
                .iter()
                .map(|parameter_id| {
                    profile
                        .parameter(parameter_id)
                        .cloned()
                        .ok_or_else(|| RuntimeError::UnknownParameter(parameter_id.clone()))
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        let session = runtime.session.as_mut().ok_or(RuntimeError::NotConnected)?;
        read_parameter_definitions(session, definitions)
    })
    .await
}

#[tauri::command]
pub async fn configure_communication(
    settings: CommunicationSettings,
    state: State<'_, AppState>,
) -> Result<(), String> {
    with_runtime(state, move |runtime| {
        settings.validate()?;
        if let Some(Session::Serial(client)) = runtime.session.as_mut() {
            client.settings = settings.clone();
        }
        runtime.communication = settings;
        runtime
            .recorder
            .set_context("communication", serde_json::json!(runtime.communication));
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn get_communication_stats(
    state: State<'_, AppState>,
) -> Result<CommunicationStats, String> {
    with_runtime(state, |runtime| {
        Ok(match runtime.session.as_ref() {
            Some(Session::Serial(client)) => client.stats.clone(),
            _ => CommunicationStats::default(),
        })
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    success: bool,
    attempts: u64,
    elapsed_ms: u64,
    error: Option<String>,
}

#[tauri::command]
pub async fn probe_read(
    address: u16,
    count: u16,
    state: State<'_, AppState>,
) -> Result<ProbeResult, String> {
    with_runtime(state, move |runtime| {
        let profile = runtime.profile.as_ref().ok_or(RuntimeError::NoProfile)?;
        // Only known contiguous status addresses may be exercised; never scan arbitrary device memory.
        if count == 0
            || count > 100
            || (0..count).any(|offset| {
                address
                    .checked_add(offset)
                    .is_none_or(|value| !profile.statuses.iter().any(|s| s.address == value))
            })
        {
            return Err(RuntimeError::Task(
                "测试范围必须是 Profile 中连续的状态寄存器（1..100）".into(),
            ));
        }
        let started = std::time::Instant::now();
        let (result, attempts) = match runtime.session.as_mut().ok_or(RuntimeError::NotConnected)? {
            Session::Serial(client) => {
                let retries = client.stats.retries;
                let result = client
                    .read_transaction(address, count)
                    .map_err(|e| e.to_string());
                (result, 1 + client.stats.retries - retries)
            }
            Session::Simulator(device) => (Ok(device.read_registers(address, count)), 1),
        };
        Ok(ProbeResult {
            success: result.is_ok(),
            attempts,
            elapsed_ms: started.elapsed().as_millis() as u64,
            error: result.err(),
        })
    })
    .await
}

#[tauri::command]
pub async fn capture_parameter_snapshot(
    label: Option<String>,
    state: State<'_, AppState>,
) -> Result<ParameterSnapshot, String> {
    with_runtime(state, move |runtime| {
        let profile = runtime.profile.clone().ok_or(RuntimeError::NoProfile)?;
        let session = runtime.session.as_mut().ok_or(RuntimeError::NotConnected)?;
        let values = read_parameter_definitions(session, profile.parameters.clone())?
            .into_iter()
            .map(|value| SnapshotValue {
                parameter_id: value.parameter_id,
                raw: value.raw,
                value: value.value,
            })
            .collect();
        let label = label
            .unwrap_or_else(|| "手动导出".into())
            .trim()
            .chars()
            .take(120)
            .collect::<String>();
        let snapshot = ParameterSnapshot {
            schema_version: "servo-parameter-snapshot/1.0".into(),
            created_at_ms: now_ms(),
            label,
            device_id: profile.device.id.clone(),
            device_name: profile.device.name.clone(),
            profile_version: profile.device.profile_version.clone(),
            values,
        };
        let snapshot_json = serde_json::to_string(&snapshot)
            .map_err(|error| RuntimeError::Snapshot(error.to_string()))?;
        runtime.audit_store.save_snapshot(
            snapshot.created_at_ms,
            &snapshot.device_id,
            &snapshot.profile_version,
            &snapshot.label,
            &snapshot_json,
        )?;
        runtime.push_audit(
            "snapshot.export",
            "success",
            &format!("{}，{} 个参数", snapshot.label, snapshot.values.len()),
        )?;
        Ok(snapshot)
    })
    .await
}

#[tauri::command]
pub async fn compare_parameter_snapshot(
    snapshot_json: String,
    state: State<'_, AppState>,
) -> Result<Vec<SnapshotDiff>, String> {
    with_runtime(state, move |runtime| {
        let snapshot: ParameterSnapshot = serde_json::from_str(&snapshot_json)
            .map_err(|error| RuntimeError::Snapshot(error.to_string()))?;
        if snapshot.schema_version != "servo-parameter-snapshot/1.0" {
            return Err(RuntimeError::Snapshot(format!(
                "不支持 schemaVersion={}",
                snapshot.schema_version
            )));
        }
        let profile = runtime.profile.clone().ok_or(RuntimeError::NoProfile)?;
        if snapshot.device_id != profile.device.id {
            return Err(RuntimeError::Snapshot(format!(
                "设备不匹配：快照为 {}，当前为 {}",
                snapshot.device_id, profile.device.id
            )));
        }
        if snapshot.profile_version != profile.device.profile_version {
            return Err(RuntimeError::Snapshot(format!(
                "Profile 版本不匹配：快照为 {}，当前为 {}",
                snapshot.profile_version, profile.device.profile_version
            )));
        }
        let mut seen = HashSet::new();
        let mut definitions = Vec::with_capacity(snapshot.values.len());
        for value in &snapshot.values {
            if !seen.insert(value.parameter_id.as_str()) {
                return Err(RuntimeError::Snapshot(format!(
                    "参数重复：{}",
                    value.parameter_id
                )));
            }
            definitions.push(
                profile
                    .parameter(&value.parameter_id)
                    .cloned()
                    .ok_or_else(|| RuntimeError::UnknownParameter(value.parameter_id.clone()))?,
            );
        }
        let session = runtime.session.as_mut().ok_or(RuntimeError::NotConnected)?;
        let current = read_parameter_definitions(session, definitions)?
            .into_iter()
            .map(|value| (value.parameter_id.clone(), value))
            .collect::<HashMap<_, _>>();
        let targets = snapshot
            .values
            .into_iter()
            .map(|value| (value.parameter_id.clone(), value))
            .collect::<HashMap<_, _>>();
        let mut diffs = Vec::with_capacity(targets.len());
        for definition in &profile.parameters {
            let Some(target) = targets.get(&definition.parameter_id) else {
                continue;
            };
            let current = current
                .get(&definition.parameter_id)
                .ok_or_else(|| RuntimeError::UnknownParameter(definition.parameter_id.clone()))?;
            let encoded_target = encode_value(definition, target.value).map_err(|error| {
                RuntimeError::Snapshot(format!("{}：{}", definition.parameter_id, error))
            })?;
            if encoded_target != target.raw {
                return Err(RuntimeError::Snapshot(format!(
                    "{} 的 raw={} 与 value={} 不一致",
                    definition.parameter_id, target.raw, target.value
                )));
            }
            diffs.push(SnapshotDiff {
                parameter_id: definition.parameter_id.clone(),
                name: definition.name.clone(),
                current_raw: current.raw,
                current_value: current.value,
                target_raw: target.raw,
                target_value: decode_value(definition.raw_type, definition.decimals, target.raw),
                changed: current.raw != target.raw,
                writable: definition.access == Access::Rw,
                risk: definition.risk,
            });
        }
        let changed = diffs.iter().filter(|diff| diff.changed).count();
        runtime.push_audit(
            "snapshot.compare",
            "success",
            &format!("{}，发现 {changed} 项差异", snapshot.label),
        )?;
        Ok(diffs)
    })
    .await
}

#[tauri::command]
pub async fn batch_write_parameters(
    request: BatchWriteRequest,
    state: State<'_, AppState>,
) -> Result<BatchWriteResult, String> {
    with_runtime(state, move |runtime| {
        if request.items.is_empty() || request.items.len() > 500 {
            return Err(RuntimeError::Confirmation(
                "批量写入数量必须为 1..500".into(),
            ));
        }
        let expected_phrase = format!("批量写入 {} 项", request.items.len());
        if request.confirmation_phrase != expected_phrase {
            return Err(RuntimeError::Confirmation(format!(
                "必须输入“{expected_phrase}”"
            )));
        }
        let profile = runtime.profile.clone().ok_or(RuntimeError::NoProfile)?;
        let mut seen = HashSet::new();
        let mut planned = Vec::with_capacity(request.items.len());
        for item in request.items {
            if !seen.insert(item.parameter_id.clone()) {
                return Err(RuntimeError::Confirmation(format!(
                    "批量列表参数重复：{}",
                    item.parameter_id
                )));
            }
            let definition = profile
                .parameter(&item.parameter_id)
                .cloned()
                .ok_or_else(|| RuntimeError::UnknownParameter(item.parameter_id.clone()))?;
            if definition.access != Access::Rw {
                return Err(RuntimeError::ReadOnly(definition.parameter_id));
            }
            let new_raw = encode_value(&definition, item.value)?;
            planned.push((definition, new_raw, item.expected_raw));
        }

        let preflight = {
            let session = runtime.session.as_mut().ok_or(RuntimeError::NotConnected)?;
            preflight_planned_writes(session, &planned)
        };
        if let Err(error) = preflight {
            runtime.push_audit(
                "parameter.batch",
                "failure",
                &format!("写前校验失败，整批未开始：{error}"),
            )?;
            return Err(error);
        }
        runtime.push_audit(
            "parameter.batch",
            "attempt",
            &format!("已完成写前校验，准备写入 {} 项", planned.len()),
        )?;

        let mut completed = Vec::with_capacity(planned.len());
        for (definition, new_raw, previous_raw) in planned {
            let result = write_planned_parameter(
                runtime.session.as_mut().ok_or(RuntimeError::NotConnected)?,
                &definition,
                new_raw,
                previous_raw,
            );
            match result {
                Ok(result) => {
                    runtime.push_audit(
                        "parameter.write",
                        "success",
                        &format!(
                            "{} raw {} -> {}，批量写入回读一致",
                            result.parameter_id, result.previous_raw, result.read_back_raw
                        ),
                    )?;
                    completed.push(result);
                }
                Err(error) => {
                    runtime.push_audit(
                        "parameter.batch",
                        "failure",
                        &format!(
                            "已完成 {} 项，{} 失败：{}",
                            completed.len(),
                            definition.parameter_id,
                            error
                        ),
                    )?;
                    return Ok(BatchWriteResult {
                        completed,
                        failed_parameter_id: Some(definition.parameter_id),
                        error: Some(error.to_string()),
                    });
                }
            }
        }
        runtime.push_audit(
            "parameter.batch",
            "success",
            &format!("{} 项全部写入并回读一致", completed.len()),
        )?;
        Ok(BatchWriteResult {
            completed,
            failed_parameter_id: None,
            error: None,
        })
    })
    .await
}

#[tauri::command]
pub async fn write_parameter(
    request: WriteRequest,
    state: State<'_, AppState>,
) -> Result<WriteResult, String> {
    with_runtime(state, move |runtime| {
        if !request.confirmed {
            return Err(RuntimeError::Confirmation("必须明确确认本次写入".into()));
        }
        let profile = runtime.profile.clone().ok_or(RuntimeError::NoProfile)?;
        let definition = profile
            .parameter(&request.parameter_id)
            .cloned()
            .ok_or_else(|| RuntimeError::UnknownParameter(request.parameter_id.clone()))?;
        if definition.access != Access::Rw {
            return Err(RuntimeError::ReadOnly(definition.parameter_id));
        }
        if matches!(definition.risk, RiskLevel::High | RiskLevel::Critical) {
            let expected_phrase = format!("写入 {}", definition.parameter_id);
            if request.confirmation_phrase.as_deref() != Some(expected_phrase.as_str()) {
                return Err(RuntimeError::Confirmation(format!(
                    "高风险参数必须输入“{expected_phrase}”"
                )));
            }
        }
        let new_raw = encode_value(&definition, request.value)?;
        let previous_raw = runtime
            .session
            .as_mut()
            .ok_or(RuntimeError::NotConnected)?
            .read_registers(definition.address, 1)?[0];
        if let Some(expected) = request.expected_raw {
            if expected != previous_raw {
                runtime.push_audit(
                    "parameter.write",
                    "failure",
                    &format!(
                        "{} 写前值变化：期望 {}，实际 {}",
                        definition.parameter_id, expected, previous_raw
                    ),
                )?;
                return Err(RuntimeError::StaleValue {
                    parameter_id: definition.parameter_id.clone(),
                    expected,
                    actual: previous_raw,
                });
            }
        }
        runtime.push_audit(
            "parameter.write",
            "attempt",
            &format!(
                "{} raw {} -> {}",
                definition.parameter_id, previous_raw, new_raw
            ),
        )?;
        let write_result = (|| {
            let session = runtime.session.as_mut().ok_or(RuntimeError::NotConnected)?;
            session.write_register(definition.address, new_raw)?;
            let read_back_raw = session.read_registers(definition.address, 1)?[0];
            if read_back_raw != new_raw {
                return Err(RuntimeError::ReadBackMismatch {
                    parameter_id: definition.parameter_id.clone(),
                    written: new_raw,
                    read_back: read_back_raw,
                });
            }
            Ok(read_back_raw)
        })();
        let read_back_raw = match write_result {
            Ok(value) => value,
            Err(error) => {
                runtime.push_audit(
                    "parameter.write",
                    "failure",
                    &format!("{}：{}", definition.parameter_id, error),
                )?;
                return Err(error);
            }
        };
        let result = WriteResult {
            parameter_id: definition.parameter_id.clone(),
            previous_raw,
            written_raw: new_raw,
            read_back_raw,
            value: decode_value(definition.raw_type, definition.decimals, read_back_raw),
        };
        runtime.push_audit(
            "parameter.write",
            "success",
            &format!(
                "{} raw {} -> {}，回读一致",
                definition.parameter_id, previous_raw, read_back_raw
            ),
        )?;
        Ok(result)
    })
    .await
}

#[tauri::command]
pub async fn apply_parameters(
    confirmation_phrase: String,
    state: State<'_, AppState>,
) -> Result<OperationResult, String> {
    if confirmation_phrase != "应用参数" {
        return Err("必须输入“应用参数”后才能执行".into());
    }
    with_runtime(state, |runtime| {
        let operations = runtime
            .profile
            .as_ref()
            .ok_or(RuntimeError::NoProfile)?
            .operations
            .clone()
            .ok_or_else(|| RuntimeError::UnsupportedOperation("应用参数".into()))?;
        let operation = operations
            .apply
            .clone()
            .ok_or_else(|| RuntimeError::UnsupportedOperation("应用参数".into()))?;
        execute_audited_operation(
            runtime,
            "parameters.apply",
            "应用参数",
            &operations,
            &operation,
        )
    })
    .await
}

#[tauri::command]
pub async fn persist_parameters(
    confirmation_phrase: String,
    state: State<'_, AppState>,
) -> Result<OperationResult, String> {
    if confirmation_phrase != "持久化参数" {
        return Err("必须输入“持久化参数”后才能执行".into());
    }
    with_runtime(state, |runtime| {
        let operations = runtime
            .profile
            .as_ref()
            .ok_or(RuntimeError::NoProfile)?
            .operations
            .clone()
            .ok_or_else(|| RuntimeError::UnsupportedOperation("持久化".into()))?;
        let operation = operations
            .persist
            .clone()
            .ok_or_else(|| RuntimeError::UnsupportedOperation("持久化".into()))?;
        execute_audited_operation(
            runtime,
            "parameters.persist",
            "持久化参数",
            &operations,
            &operation,
        )
    })
    .await
}

#[tauri::command]
pub async fn read_statuses(state: State<'_, AppState>) -> Result<Vec<StatusValue>, String> {
    with_runtime(state, |runtime| {
        let statuses = runtime
            .profile
            .as_ref()
            .ok_or(RuntimeError::NoProfile)?
            .statuses
            .clone();
        let session = runtime.session.as_mut().ok_or(RuntimeError::NotConnected)?;
        let simulator = session.mode() == ConnectionMode::Simulator;
        let values = read_status_definitions(session, statuses)?;
        if simulator {
            runtime.recorder.emit(
                "simulator",
                "status-sample",
                "event",
                0,
                &[],
                &serde_json::json!(values).to_string(),
            );
        }
        Ok(values)
    })
    .await
}

#[tauri::command]
pub async fn get_audit_log(state: State<'_, AppState>) -> Result<Vec<AuditEntry>, String> {
    with_runtime(state, |runtime| Ok(runtime.audit_store.list(500)?)).await
}

fn read_parameter_definitions(
    session: &mut Session,
    mut definitions: Vec<ParameterDefinition>,
) -> Result<Vec<ParameterValue>, RuntimeError> {
    definitions.sort_by_key(|definition| definition.address);
    let mut values = Vec::with_capacity(definitions.len());
    let mut cursor = 0;
    while cursor < definitions.len() {
        let start = definitions[cursor].address;
        let mut end = cursor + 1;
        while end < definitions.len()
            && definitions[end].address == definitions[end - 1].address + 1
            && definitions[end].address - start < 100
        {
            end += 1;
        }
        let count = definitions[end - 1].address - start + 1;
        let registers = session.read_registers(start, count)?;
        for definition in &definitions[cursor..end] {
            let raw = registers[(definition.address - start) as usize];
            values.push(ParameterValue {
                parameter_id: definition.parameter_id.clone(),
                raw,
                value: decode_value(definition.raw_type, definition.decimals, raw),
            });
        }
        cursor = end;
    }
    Ok(values)
}

fn read_status_definitions(
    session: &mut Session,
    mut definitions: Vec<StatusDefinition>,
) -> Result<Vec<StatusValue>, RuntimeError> {
    definitions.sort_by_key(|definition| definition.address);
    let mut values = Vec::with_capacity(definitions.len());
    let mut cursor = 0;
    while cursor < definitions.len() {
        let start = definitions[cursor].address;
        let mut end = cursor + 1;
        while end < definitions.len()
            && definitions[end].address == definitions[end - 1].address + 1
            && definitions[end].address - start < 100
        {
            end += 1;
        }
        let count = definitions[end - 1].address - start + 1;
        let registers = session.read_registers(start, count)?;
        for definition in &definitions[cursor..end] {
            let raw = registers[(definition.address - start) as usize];
            values.push(StatusValue {
                id: definition.id.clone(),
                name: definition.name.clone(),
                address: definition.address,
                raw,
                value: decode_value(definition.raw_type, definition.decimals, raw),
                unit: definition.unit.clone(),
            });
        }
        cursor = end;
    }
    Ok(values)
}

fn preflight_planned_writes(
    session: &mut Session,
    planned: &[PlannedWrite],
) -> Result<(), RuntimeError> {
    let current_values = read_parameter_definitions(
        session,
        planned
            .iter()
            .map(|(definition, _, _)| definition.clone())
            .collect(),
    )?
    .into_iter()
    .map(|value| (value.parameter_id, value.raw))
    .collect::<HashMap<_, _>>();
    for (definition, _, expected_raw) in planned {
        let actual = *current_values
            .get(&definition.parameter_id)
            .ok_or_else(|| RuntimeError::UnknownParameter(definition.parameter_id.clone()))?;
        if actual != *expected_raw {
            return Err(RuntimeError::StaleValue {
                parameter_id: definition.parameter_id.clone(),
                expected: *expected_raw,
                actual,
            });
        }
    }
    Ok(())
}

fn write_planned_parameter(
    session: &mut Session,
    definition: &ParameterDefinition,
    new_raw: u16,
    previous_raw: u16,
) -> Result<WriteResult, RuntimeError> {
    session.write_register(definition.address, new_raw)?;
    let read_back = session.read_registers(definition.address, 1)?[0];
    if read_back != new_raw {
        return Err(RuntimeError::ReadBackMismatch {
            parameter_id: definition.parameter_id.clone(),
            written: new_raw,
            read_back,
        });
    }
    Ok(WriteResult {
        parameter_id: definition.parameter_id.clone(),
        previous_raw,
        written_raw: new_raw,
        read_back_raw: read_back,
        value: decode_value(definition.raw_type, definition.decimals, read_back),
    })
}

fn execute_operation(
    runtime: &mut Runtime,
    name: &str,
    operations: &OperationSet,
    operation: &OperationDefinition,
) -> Result<OperationResult, RuntimeError> {
    let session = runtime.session.as_mut().ok_or(RuntimeError::NotConnected)?;
    session.write_register(operations.command_register, operation.command)?;
    let started = std::time::Instant::now();
    while started.elapsed() <= Duration::from_millis(operation.timeout_ms) {
        let observed = session.read_registers(operations.status_register, 1)?[0];
        if observed == operation.success_status {
            return Ok(OperationResult {
                operation: name.into(),
                command: operation.command,
                observed_status: observed,
            });
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(RuntimeError::OperationTimeout(name.into()))
}

fn execute_audited_operation(
    runtime: &mut Runtime,
    action: &str,
    name: &str,
    operations: &OperationSet,
    operation: &OperationDefinition,
) -> Result<OperationResult, RuntimeError> {
    runtime.push_audit(
        action,
        "attempt",
        &format!("写入命令 0x{:04X}", operation.command),
    )?;
    match execute_operation(runtime, name, operations, operation) {
        Ok(result) => {
            runtime.push_audit(
                action,
                "success",
                &format!("设备返回 0x{:04X}", result.observed_status),
            )?;
            Ok(result)
        }
        Err(error) => {
            runtime.push_audit(action, "failure", &error.to_string())?;
            Err(error)
        }
    }
}

fn profile_summary(profile: &ServoProfile) -> ProfileSummary {
    ProfileSummary {
        device_id: profile.device.id.clone(),
        device_name: profile.device.name.clone(),
        profile_version: profile.device.profile_version.clone(),
        parameter_count: profile.parameters.len(),
        status_count: profile.statuses.len(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{
        Access, DeviceInfo, ParameterDefinition, RawType, RiskLevel, StatusDefinition,
        TransportProfile,
    };

    #[test]
    fn serial_presets_validate_limits_and_preserve_legacy_requests() {
        let profile = test_profile();
        let mut request: ConnectionRequest = serde_json::from_value(serde_json::json!({"mode":"serial","portName":"TEST","slaveId":1,"baudRate":19200,"parity":"even","stopBits":1,"timeoutMs":200})).unwrap();
        assert_eq!(request.protocol, Protocol::Rtu);
        request.slave_id = 247;
        assert!(request.validate(&profile).is_ok());
        request.preset = ConnectionPreset::P300;
        assert!(request.validate(&profile).is_err());
        request.slave_id = 32;
        for rate in [4800, 9600, 19200, 38400, 57600, 115200] {
            request.baud_rate = rate;
            assert!(request.validate(&profile).is_ok());
        }
        request.baud_rate = 0;
        assert!(request.validate(&profile).is_err());
        request.baud_rate = 19200;
        request.stop_bits = 2;
        assert!(request.validate(&profile).is_err());
    }

    fn test_profile() -> ServoProfile {
        ServoProfile {
            schema_version: "1.0".into(),
            device: DeviceInfo {
                id: "test".into(),
                name: "Test".into(),
                profile_version: "1".into(),
            },
            transport: TransportProfile {
                kind: "modbus-rtu".into(),
                default_slave_id: 1,
                default_baud_rate: 19_200,
                allowed_baud_rates: vec![19_200],
                data_bits: 8,
                parity: ParitySetting::Even,
                stop_bits: 1,
                timeout_ms: 500,
            },
            parameters: vec![ParameterDefinition {
                semantic_id: "speed.gain".into(),
                parameter_id: "gain-main".into(),
                name: "速度增益".into(),
                group: "速度环".into(),
                address: 5,
                raw_type: RawType::I16,
                decimals: 0,
                unit: "Hz".into(),
                min: 1.0,
                max: 3000.0,
                default_value: 40.0,
                access: Access::Rw,
                risk: RiskLevel::Medium,
                requires_restart: false,
                applicable_modes: vec!["position".into(), "speed".into()],
                enum_values: vec![],
                description: String::new(),
            }],
            statuses: vec![StatusDefinition {
                id: "speed".into(),
                name: "速度".into(),
                address: 0x1000,
                raw_type: RawType::I16,
                decimals: 0,
                unit: "r/min".into(),
            }],
            operations: Some(OperationSet {
                command_register: 0x1100,
                status_register: 0x1101,
                apply: Some(OperationDefinition {
                    command: 0xBB00,
                    success_status: 0x44FF,
                    timeout_ms: 100,
                }),
                persist: Some(OperationDefinition {
                    command: 0x0011,
                    success_status: 0xFFEE,
                    timeout_ms: 100,
                }),
            }),
        }
    }

    #[test]
    fn simulator_supports_read_write_apply_and_persist() {
        let profile = test_profile();
        let mut session = Session::Simulator(SimulatorDevice::new(&profile).unwrap());
        let operations = profile.operations.as_ref().unwrap();
        assert_eq!(session.read_registers(5, 1).unwrap(), vec![40]);
        session.write_register(5, 55).unwrap();
        assert_eq!(session.read_registers(5, 1).unwrap(), vec![55]);
        session
            .write_register(operations.command_register, 0xBB00)
            .unwrap();
        assert_eq!(
            session
                .read_registers(operations.status_register, 1)
                .unwrap(),
            vec![0x44FF]
        );
        session
            .write_register(operations.command_register, 0x0011)
            .unwrap();
        assert_eq!(
            session
                .read_registers(operations.status_register, 1)
                .unwrap(),
            vec![0xFFEE]
        );
    }

    #[test]
    fn grouped_reads_cover_configured_parameters_and_statuses() {
        let profile = test_profile();
        let mut session = Session::Simulator(SimulatorDevice::new(&profile).unwrap());
        let parameters = read_parameter_definitions(&mut session, profile.parameters.clone())
            .expect("complete parameter read should succeed");
        let statuses = read_status_definitions(&mut session, profile.statuses.clone())
            .expect("complete status read should succeed");

        assert_eq!(parameters.len(), 1);
        assert_eq!(statuses.len(), 1);
        assert_eq!(
            parameters
                .iter()
                .find(|value| value.parameter_id == "gain-main")
                .unwrap()
                .value,
            40.0
        );
        assert_eq!(
            statuses
                .iter()
                .find(|value| value.id == "speed")
                .unwrap()
                .value,
            0.0
        );
    }

    #[test]
    fn batch_preflight_rejects_stale_values_before_writing() {
        let profile = test_profile();
        let definition = profile.parameters[0].clone();
        let mut session = Session::Simulator(SimulatorDevice::new(&profile).unwrap());
        let stale = vec![(definition.clone(), 55, 41)];

        assert!(matches!(
            preflight_planned_writes(&mut session, &stale),
            Err(RuntimeError::StaleValue { .. })
        ));
        assert_eq!(
            session.read_registers(definition.address, 1).unwrap(),
            vec![40]
        );

        let current = vec![(definition.clone(), 55, 40)];
        preflight_planned_writes(&mut session, &current).unwrap();
        let result = write_planned_parameter(&mut session, &definition, 55, 40).unwrap();
        assert_eq!(result.read_back_raw, 55);
    }
}
