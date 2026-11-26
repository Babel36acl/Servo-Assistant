//! 自动查找：按站号与波特率计划探测；BLE 桥在探测中在线重配而不重连。
use super::connect::open_serial_client;
use super::types::{ConnectionMode, ConnectionPreset, ConnectionRequest};
use super::AppState;
use crate::discovery::{self, DiscoveryStatus};
use crate::modbus::SerialClient;
use serde::Deserialize;
use std::sync::atomic::Ordering;
use tauri::State;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryRequest {
    connection: ConnectionRequest,
    start_slave: u8,
    end_slave: u8,
    /// Optional explicit baud list for this scan. When absent the Profile (or
    /// P300 preset) list is used; the preferred baud must be included.
    #[serde(default)]
    baud_rates: Option<Vec<u32>>,
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
    if state.scan.running.load(Ordering::SeqCst) {
        state.discovery.running.store(false, Ordering::SeqCst);
        return Err("寄存器扫描占用串口，请先取消或等待完成".into());
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
        if !matches!(connection.mode, ConnectionMode::Serial | ConnectionMode::Ble) { return Err("自动查找仅适用于真实串口或蓝牙桥".into()); }
        if request.start_slave == 0 || request.start_slave > request.end_slave || request.end_slave > 247 {
            return Err("站号范围须为 1..247".into());
        }
        if connection.preset == ConnectionPreset::P300 && request.end_slave > 32 { return Err("P300 站号范围为 1..32".into()); }
        let allowed = connection.allowed_rates(profile);
        let rates = match request.baud_rates {
            Some(selected) => discovery::validate_selected_rates(connection.baud_rate, &selected, &allowed)?,
            None => allowed,
        };
        connection.validate_with_rates(&rates)?;
        let preferred = discovery::SerialMode { protocol: connection.protocol, parity: connection.parity, stop_bits: connection.stop_bits };
        let modes = discovery::modes(preferred);
        let candidates = discovery::plan(
            request.start_slave,
            request.end_slave,
            connection.slave_id,
            connection.baud_rate,
            &rates,
            &modes,
        );
        let address = profile.statuses.first().map(|item| item.address)
            .or_else(|| profile.parameters.first().map(|item| item.address)).ok_or("Profile 没有可读取的地址")?;
        let is_ble = matches!(connection.mode, ConnectionMode::Ble);
        let mut client: Option<(u32, discovery::SerialMode, SerialClient)> = None;
        let result = discovery::scan(&worker_control, &candidates, |baud, slave, mode| {
            if client.as_ref().is_none_or(|(rate, current, _)| *rate != baud || *current != mode) {
                // BLE：拆出旧句柄在线重配（transport 已含录制包装，不重复包），不重连；
                // 串口：关旧句柄按新参数重开。
                let existing = match client.take() {
                    Some((_, _, previous)) if is_ble => Some(previous.into_transport()),
                    _ => None,
                };
                connection.protocol = mode.protocol;
                connection.parity = mode.parity;
                connection.stop_bits = mode.stop_bits;
                connection.baud_rate = baud;
                connection.slave_id = slave;
                let mut rebuilt = if is_ble {
                    let mut transport = existing
                        .ok_or_else(|| "蓝牙链路丢失，请重试自动查找".to_string())?;
                    transport
                        .reconfigure(baud, mode.parity, mode.stop_bits)
                        .map_err(|error| error.to_string())?;
                    let bits = crate::ble::bits_per_char(mode.parity, mode.stop_bits);
                    SerialClient::new(transport, slave, baud, bits)
                } else {
                    open_serial_client(&connection, &runtime.recorder)
                        .map_err(|error| error.to_string())?
                };
                rebuilt.set_protocol(mode.protocol);
                client = Some((baud, mode, rebuilt));
            }
            client.as_mut().unwrap().2.detect_slave(slave, address).map_err(|error| error.to_string())
        });
        drop(client);
        if is_ble {
            crate::ble::disconnect_all();
        }
        let detail = match &result {
            Ok(status) => format!("port={:?} checked={}/{} found={} cancelled={} slave={:?} baud={:?} mode={:?} rates={rates:?} address=0x{address:04X}", connection.port_name, status.completed, status.total, status.found, status.cancelled, status.slave_id, status.baud_rate, status.serial_mode),
            Err(error) => error.clone(),
        };
        runtime.push_audit("communication.discovery", if result.is_ok() { "completed" } else { "failed" }, &detail).map_err(|error| error.to_string())?;
        result
    }).await.map_err(|error| error.to_string()).and_then(|result| result);
    result
}
