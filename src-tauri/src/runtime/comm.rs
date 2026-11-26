//! 通讯设置与链路探测：设置只作用于当前会话，探测只读 Profile 状态寄存器。
use super::{with_runtime, AppState, RuntimeError, Session};
use crate::modbus::{CommunicationSettings, CommunicationStats};
use serde::Serialize;
use tauri::State;

#[tauri::command]
pub async fn configure_communication(
    settings: CommunicationSettings,
    state: State<'_, AppState>,
) -> Result<(), String> {
    with_runtime(state, move |runtime| {
        settings.validate()?;
        if let Some(session) = runtime.session.as_mut() {
            session.set_settings(settings.clone());
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
            Some(Session::Ble(client)) => client.stats.clone(),
            Some(Session::Tcp(client)) => client.stats.clone(),
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
            Session::Tcp(client) => {
                let retries = client.stats.retries;
                let result = client
                    .read_transaction(address, count)
                    .map_err(|e| e.to_string());
                (result, 1 + client.stats.retries - retries)
            }
            Session::Ble(client) => {
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
