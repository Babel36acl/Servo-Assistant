//! 寄存器扫描：只读 FC03 扫描，独占临时句柄，绝不下发写操作。
use super::connect::{open_ble_client, open_serial_client, open_tcp_client};
use super::types::{ConnectionMode, ConnectionRequest};
use super::AppState;
use crate::modbus::{ModbusError, SerialClient};
use crate::modbus_tcp::TcpClient;
use crate::scan::{self, ScanStatus};
use serde::Deserialize;
use std::sync::atomic::Ordering;
use tauri::State;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterScanRequest {
    connection: ConnectionRequest,
    start_address: u16,
    end_address: u16,
    block_size: u16,
    #[serde(default)]
    include_zero: bool,
}

/// 寄存器扫描的临时句柄。Serial 变体同样承载 BLE 桥会话：
/// 两者都是 SerialClient，走同一条 read_transaction 路径。
enum ScanTransport {
    Serial(SerialClient),
    Tcp(TcpClient),
}
impl ScanTransport {
    fn read_transaction(&mut self, address: u16, count: u16) -> Result<Vec<u16>, ModbusError> {
        match self {
            Self::Serial(client) => client.read_transaction(address, count),
            Self::Tcp(client) => client.read_transaction(address, count),
        }
    }
}

#[tauri::command]
pub fn register_scan_status(state: State<'_, AppState>) -> Result<ScanStatus, String> {
    state
        .scan
        .status
        .lock()
        .map(|status| status.clone())
        .map_err(|_| "扫描状态锁已损坏".into())
}

#[tauri::command]
pub fn cancel_register_scan(state: State<'_, AppState>) {
    state.scan.cancel.store(true, Ordering::SeqCst);
}

/// Read-only FC03 scan over an explicit address range. The scan owns a
/// temporary serial or TCP handle and refuses to run while a device session or
/// the auto-discovery scan owns the link. It never writes to the device.
#[tauri::command]
pub async fn scan_registers(
    request: RegisterScanRequest,
    state: State<'_, AppState>,
) -> Result<ScanStatus, String> {
    if state
        .scan
        .running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("寄存器扫描已经运行".into());
    }
    if state.discovery.running.load(Ordering::SeqCst) {
        state.scan.running.store(false, Ordering::SeqCst);
        return Err("自动查找占用串口，请先取消或等待完成".into());
    }
    state.scan.cancel.store(false, Ordering::SeqCst);
    if let Ok(mut status) = state.scan.status.lock() {
        *status = ScanStatus::default();
    }
    let control = state.scan.clone();
    let inner = state.inner.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<ScanStatus, String> {
        let _lease = scan::ScanLease(control.clone());
        let connection = request.connection;
        if !(1..=247).contains(&connection.slave_id) {
            return Err("站号必须为 1..247".into());
        }
        if !(100..=10_000).contains(&connection.timeout_ms) {
            return Err("超时须为 100..10000 ms".into());
        }
        scan::blocks(request.start_address, request.end_address, request.block_size)?;
        let recorder = {
            let runtime = inner.lock().map_err(|_| "内部状态锁已损坏")?;
            if runtime.session.is_some() {
                return Err("请先断开连接再扫描寄存器".into());
            }
            runtime.recorder.clone()
        };
        let (mut client, endpoint) = match connection.mode {
            ConnectionMode::Serial => {
                if !(1200..=1_000_000).contains(&connection.baud_rate) {
                    return Err("波特率须为 1200..1000000".into());
                }
                if !matches!(connection.stop_bits, 1 | 2) {
                    return Err("停止位只能为 1 或 2".into());
                }
                let client = open_serial_client(&connection, &recorder)
                    .map_err(|error| error.to_string())?;
                (ScanTransport::Serial(client), format!("port={:?}", connection.port_name))
            }
            ConnectionMode::Tcp => {
                connection.validate_tcp_endpoint()?;
                let client =
                    open_tcp_client(&connection, &recorder).map_err(|error| error.to_string())?;
                (
                    ScanTransport::Tcp(client),
                    format!("host={:?} port={:?}", connection.host, connection.port),
                )
            }
            ConnectionMode::Ble => {
                if !(1200..=1_000_000).contains(&connection.baud_rate) {
                    return Err("波特率须为 1200..1000000".into());
                }
                if !matches!(connection.stop_bits, 1 | 2) {
                    return Err("停止位只能为 1 或 2".into());
                }
                let address = connection
                    .ble_device
                    .as_deref()
                    .map(str::trim)
                    .filter(|address| !address.is_empty())
                    .ok_or("必须选择蓝牙桥设备")?;
                let client = open_ble_client(&connection, &recorder, true)
                    .map_err(|error| error.to_string())?;
                (ScanTransport::Serial(client), format!("ble={address:?}"))
            }
            ConnectionMode::Simulator => {
                return Err("寄存器扫描仅支持真实串口或 Modbus TCP".into());
            }
        };
        let status = scan::scan(
            &control,
            request.start_address,
            request.end_address,
            request.block_size,
            request.include_zero,
            |address, count| client.read_transaction(address, count).map_err(|error| error.to_string()),
        )?;
        let detail = format!(
            "{endpoint} slave={} range=0x{:04X}..0x{:04X} block={} values={} failed_blocks={} cancelled={} elapsed_ms={}",
            connection.slave_id,
            request.start_address,
            request.end_address,
            request.block_size,
            status.values.len(),
            status.failed_blocks,
            status.cancelled,
            status.elapsed_ms
        );
        let mut runtime = inner.lock().map_err(|_| "内部状态锁已损坏")?;
        runtime
            .push_audit(
                "communication.register_scan",
                if status.cancelled { "cancelled" } else { "completed" },
                &detail,
            )
            .map_err(|error| error.to_string())?;
        Ok(status)
    })
    .await
    .map_err(|error| error.to_string())
    .and_then(|result| result);
    result
}
