//! IPC 数据结构：serde 字段名就是前端契约，改动必须同步前端与 IPC 权限测试。
use crate::discovery;
use crate::modbus::Protocol;
use crate::profile::{ParitySetting, RiskLevel, ServoProfile};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionMode {
    Simulator,
    Serial,
    Ble,
    Tcp,
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
    pub(super) fn allowed_rates(&self, profile: &ServoProfile) -> Vec<u32> {
        let mut rates = self.rates(profile);
        if self.allow_unsupported_baud {
            for rate in discovery::COMMON_BAUD_RATES {
                if !rates.contains(&rate) {
                    rates.push(rate);
                }
            }
        }
        rates
    }
    pub(super) fn validate_tcp_endpoint(&self) -> Result<(), String> {
        let host = self.host.as_deref().map(str::trim).unwrap_or_default();
        if host.is_empty() || host.len() > 255 {
            return Err("Modbus TCP 主机须为 1..255 个字符".into());
        }
        if !(1..=65535).contains(&self.port.unwrap_or(0)) {
            return Err("Modbus TCP 端口须为 1..65535".into());
        }
        if !(1..=247).contains(&self.slave_id) {
            return Err("站号必须为 1..247".into());
        }
        Ok(())
    }
    pub(super) fn validate(&self, profile: &ServoProfile) -> Result<(), String> {
        let rates = self.allowed_rates(profile);
        self.validate_with_rates(&rates)
    }
    pub(super) fn validate_with_rates(&self, rates: &[u32]) -> Result<(), String> {
        if self.mode == ConnectionMode::Tcp {
            return self.validate_tcp_endpoint();
        }
        let max = if self.preset == ConnectionPreset::P300 {
            32
        } else {
            247
        };
        if !(1..=max).contains(&self.slave_id) {
            return Err(format!("站号必须为 1..{max}"));
        }
        if !rates.contains(&self.baud_rate) || self.baud_rate == 0 {
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
    /// BLE 桥设备地址；`mode` 为 `ble` 时使用。
    #[serde(default)]
    pub ble_device: Option<String>,
    /// Modbus TCP endpoint; only used when `mode` is `tcp`.
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    pub slave_id: u8,
    pub baud_rate: u32,
    /// Explicit, session-only opt-in to link rates outside the Profile list.
    /// It never changes what a Profile declares; connection and discovery both
    /// have to receive this flag to use an extended rate.
    #[serde(default)]
    pub allow_unsupported_baud: bool,
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
