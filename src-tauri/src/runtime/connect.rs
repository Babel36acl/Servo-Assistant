//! 连接生命周期：导入 Profile、枚举串口、打开串口/TCP/BLE 会话与断开。
use super::simulator::SimulatorDevice;
use super::types::{
    ConnectionMode, ConnectionRequest, ConnectionStatus, ProfileSummary, SerialPortInfo,
};
use super::{connection_status, with_runtime, AppState, RuntimeError, Session};
use crate::modbus::{SerialClient, SerialTransport};
use crate::modbus_tcp::TcpClient;
use crate::profile::{ParitySetting, ProfileError, ServoProfile};
use serialport::{DataBits, Parity, StopBits};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
use tauri::State;

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

pub(super) fn open_serial_client(
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
    Ok(SerialClient::new(
        Box::new(SerialTransport(port)),
        request.slave_id,
        request.baud_rate,
        bits_per_char,
    )
    .with_protocol(request.protocol)
    .with_recorder(recorder.clone()))
}

pub(super) fn open_tcp_client(
    request: &ConnectionRequest,
    recorder: &crate::recording::Recorder,
) -> Result<TcpClient, RuntimeError> {
    let host = request
        .host
        .as_deref()
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .ok_or_else(|| {
            RuntimeError::Profile(ProfileError::Validation("必须填写 Modbus TCP 主机".into()))
        })?;
    let port = request.port.ok_or_else(|| {
        RuntimeError::Profile(ProfileError::Validation("必须填写 Modbus TCP 端口".into()))
    })?;
    let timeout = Duration::from_millis(request.timeout_ms.clamp(100, 10_000));
    let mut last_error = None;
    let mut stream = None;
    for address in (host, port)
        .to_socket_addrs()
        .map_err(|error| RuntimeError::Task(format!("无法解析 {host}:{port}：{error}")))?
    {
        match TcpStream::connect_timeout(&address, timeout) {
            Ok(value) => {
                stream = Some(value);
                break;
            }
            Err(error) => last_error = Some(error),
        }
    }
    let stream = stream.ok_or_else(|| {
        RuntimeError::Task(format!(
            "无法连接 {host}:{port}：{}",
            last_error
                .map(|error| error.to_string())
                .unwrap_or_else(|| "地址不可用".into())
        ))
    })?;
    Ok(TcpClient::new(
        stream,
        request.slave_id,
        timeout,
        format!("tcp:{host}:{port}"),
        Some(recorder.clone()),
    ))
}

pub(super) fn open_ble_client(
    request: &ConnectionRequest,
    recorder: &crate::recording::Recorder,
    disconnect_on_drop: bool,
) -> Result<SerialClient, RuntimeError> {
    let address = request
        .ble_device
        .as_deref()
        .map(str::trim)
        .filter(|address| !address.is_empty())
        .ok_or_else(|| {
            RuntimeError::Profile(ProfileError::Validation("必须选择蓝牙桥设备".into()))
        })?;
    let timeout = Duration::from_millis(request.timeout_ms.clamp(100, 10_000));
    crate::ble::open_client(
        crate::ble::BleConnectOptions {
            address: address.to_string(),
            slave_id: request.slave_id,
            protocol: request.protocol,
            baud: request.baud_rate,
            parity: request.parity,
            stop_bits: request.stop_bits,
            timeout,
            disconnect_on_drop,
        },
        recorder,
    )
    .map_err(RuntimeError::Task)
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
            ConnectionMode::Tcp => Session::Tcp(open_tcp_client(&request, &runtime.recorder)?),
            ConnectionMode::Ble => {
                Session::Ble(open_ble_client(&request, &runtime.recorder, true)?)
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
        let settings = runtime.communication.clone();
        if let Some(session) = runtime.session.as_mut() {
            session.set_settings(settings);
        }
        let endpoint = match mode {
            ConnectionMode::Tcp => format!(", host={:?}, port={:?}", request.host, request.port),
            _ => String::new(),
        };
        runtime.push_audit(
            "connection.open",
            "success",
            &format!("mode={mode:?}, slave={}{endpoint}", request.slave_id),
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
pub async fn ble_scan(
    name_contains: Option<String>,
    timeout_ms: Option<u64>,
) -> Result<Vec<crate::ble::BleDeviceInfo>, String> {
    let timeout_ms = timeout_ms.unwrap_or(4000).clamp(1000, 30_000);
    crate::ble::scan_devices(name_contains, timeout_ms).await
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

fn profile_summary(profile: &ServoProfile) -> ProfileSummary {
    ProfileSummary {
        device_id: profile.device.id.clone(),
        device_name: profile.device.name.clone(),
        profile_version: profile.device.profile_version.clone(),
        parameter_count: profile.parameters.len(),
        status_count: profile.statuses.len(),
    }
}
