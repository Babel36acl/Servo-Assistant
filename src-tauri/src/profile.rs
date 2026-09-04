use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("配置文件 JSON 无效：{0}")]
    Json(#[from] serde_json::Error),
    #[error("配置文件校验失败：{0}")]
    Validation(String),
    #[error("数值 {value} 无法按 {decimals} 位小数精确编码")]
    Precision { value: f64, decimals: u8 },
    #[error("数值 {value} 超出 {raw_type:?} 的通讯范围")]
    RawRange { value: f64, raw_type: RawType },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServoProfile {
    pub schema_version: String,
    pub device: DeviceInfo,
    pub transport: TransportProfile,
    pub parameters: Vec<ParameterDefinition>,
    #[serde(default)]
    pub statuses: Vec<StatusDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operations: Option<OperationSet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub profile_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportProfile {
    pub kind: String,
    pub default_slave_id: u8,
    pub default_baud_rate: u32,
    pub allowed_baud_rates: Vec<u32>,
    pub data_bits: u8,
    pub parity: ParitySetting,
    pub stop_bits: u8,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParitySetting {
    None,
    Even,
    Odd,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RawType {
    U16,
    I16,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    Ro,
    Rw,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumChoice {
    pub value: f64,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterDefinition {
    pub semantic_id: String,
    pub parameter_id: String,
    pub name: String,
    pub group: String,
    pub address: u16,
    pub raw_type: RawType,
    pub decimals: u8,
    pub unit: String,
    pub min: f64,
    pub max: f64,
    pub default_value: f64,
    pub access: Access,
    pub risk: RiskLevel,
    #[serde(default)]
    pub requires_restart: bool,
    #[serde(default)]
    pub applicable_modes: Vec<String>,
    #[serde(default)]
    pub enum_values: Vec<EnumChoice>,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusDefinition {
    pub id: String,
    pub name: String,
    pub address: u16,
    pub raw_type: RawType,
    pub decimals: u8,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationSet {
    pub command_register: u16,
    pub status_register: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply: Option<OperationDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persist: Option<OperationDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationDefinition {
    pub command: u16,
    pub success_status: u16,
    pub timeout_ms: u64,
}

impl ServoProfile {
    pub fn from_json(json: &str) -> Result<Self, ProfileError> {
        let profile: ServoProfile = serde_json::from_str(json)?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.schema_version != "1.0" {
            return Err(ProfileError::Validation(format!(
                "不支持 schemaVersion={}，当前只接受 1.0",
                self.schema_version
            )));
        }
        if self.device.id.trim().is_empty() || self.device.name.trim().is_empty() {
            return Err(ProfileError::Validation("设备 id 和 name 不能为空".into()));
        }
        if self.transport.kind != "modbus-rtu" {
            return Err(ProfileError::Validation(
                "V1 仅支持 transport.kind=modbus-rtu".into(),
            ));
        }
        if !(1..=247).contains(&self.transport.default_slave_id) {
            return Err(ProfileError::Validation("Modbus 站号必须为 1..247".into()));
        }
        if self.transport.data_bits != 8 || !matches!(self.transport.stop_bits, 1 | 2) {
            return Err(ProfileError::Validation(
                "当前仅支持 8 数据位和 1/2 停止位".into(),
            ));
        }
        if !self
            .transport
            .allowed_baud_rates
            .contains(&self.transport.default_baud_rate)
        {
            return Err(ProfileError::Validation(
                "defaultBaudRate 必须出现在 allowedBaudRates 中".into(),
            ));
        }
        if self.parameters.is_empty() {
            return Err(ProfileError::Validation("参数表不能为空".into()));
        }

        let mut semantic_ids = HashSet::new();
        let mut parameter_ids = HashSet::new();
        let mut addresses = HashSet::new();
        for parameter in &self.parameters {
            if !semantic_ids.insert(parameter.semantic_id.as_str()) {
                return Err(ProfileError::Validation(format!(
                    "semanticId 重复：{}",
                    parameter.semantic_id
                )));
            }
            if parameter.parameter_id.trim().is_empty() {
                return Err(ProfileError::Validation("parameterId 不能为空".into()));
            }
            if !parameter_ids.insert(parameter.parameter_id.as_str()) {
                return Err(ProfileError::Validation(format!(
                    "parameterId 重复：{}",
                    parameter.parameter_id
                )));
            }
            if !addresses.insert(parameter.address) {
                return Err(ProfileError::Validation(format!(
                    "参数通讯地址重复：0x{:04X}",
                    parameter.address
                )));
            }
            if parameter.decimals > 6 {
                return Err(ProfileError::Validation(format!(
                    "{} 的 decimals 不能大于 6",
                    parameter.parameter_id
                )));
            }
            if parameter.min > parameter.max
                || parameter.default_value < parameter.min
                || parameter.default_value > parameter.max
            {
                return Err(ProfileError::Validation(format!(
                    "{} 的范围或默认值无效",
                    parameter.parameter_id
                )));
            }
            encode_value(parameter, parameter.min)?;
            encode_value(parameter, parameter.max)?;
            encode_value(parameter, parameter.default_value)?;
            for choice in &parameter.enum_values {
                if choice.value < parameter.min || choice.value > parameter.max {
                    return Err(ProfileError::Validation(format!(
                        "{} 的枚举值 {} 超出范围",
                        parameter.parameter_id, choice.value
                    )));
                }
                encode_value(parameter, choice.value)?;
            }
        }

        let mut status_ids = HashSet::new();
        let mut status_addresses = HashSet::new();
        for status in &self.statuses {
            if !status_ids.insert(status.id.as_str()) {
                return Err(ProfileError::Validation(format!(
                    "状态 id 重复：{}",
                    status.id
                )));
            }
            if !status_addresses.insert(status.address) {
                return Err(ProfileError::Validation(format!(
                    "状态地址重复：0x{:04X}",
                    status.address
                )));
            }
            if status.decimals > 6 {
                return Err(ProfileError::Validation(format!(
                    "状态 {} 的 decimals 不能大于 6",
                    status.id
                )));
            }
        }
        if let Some(operations) = &self.operations {
            if operations.command_register == operations.status_register {
                return Err(ProfileError::Validation(
                    "操作命令寄存器和状态寄存器不能相同".into(),
                ));
            }
            if operations.apply.is_none() && operations.persist.is_none() {
                return Err(ProfileError::Validation(
                    "operations 至少需要定义 apply 或 persist".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn parameter(&self, parameter_id: &str) -> Option<&ParameterDefinition> {
        self.parameters
            .iter()
            .find(|item| item.parameter_id == parameter_id)
    }
}

pub fn encode_value(parameter: &ParameterDefinition, value: f64) -> Result<u16, ProfileError> {
    if !value.is_finite() || value < parameter.min || value > parameter.max {
        return Err(ProfileError::Validation(format!(
            "{} 数值 {} 超出范围 {}..{}",
            parameter.parameter_id, value, parameter.min, parameter.max
        )));
    }
    let factor = 10_f64.powi(parameter.decimals as i32);
    let scaled = value * factor;
    let rounded = scaled.round();
    if (scaled - rounded).abs() > 1e-7 {
        return Err(ProfileError::Precision {
            value,
            decimals: parameter.decimals,
        });
    }
    match parameter.raw_type {
        RawType::U16 if (0.0..=u16::MAX as f64).contains(&rounded) => Ok(rounded as u16),
        RawType::I16 if (i16::MIN as f64..=i16::MAX as f64).contains(&rounded) => {
            Ok((rounded as i16) as u16)
        }
        raw_type => Err(ProfileError::RawRange { value, raw_type }),
    }
}

pub fn decode_value(raw_type: RawType, decimals: u8, raw: u16) -> f64 {
    let signed = match raw_type {
        RawType::U16 => raw as f64,
        RawType::I16 => (raw as i16) as f64,
    };
    signed / 10_f64.powi(decimals as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parameter(raw_type: RawType, decimals: u8, min: f64, max: f64) -> ParameterDefinition {
        ParameterDefinition {
            semantic_id: "test.value".into(),
            parameter_id: "gain-01".into(),
            name: "测试".into(),
            group: "test".into(),
            address: 0x012D,
            raw_type,
            decimals,
            unit: String::new(),
            min,
            max,
            default_value: 0.0,
            access: Access::Rw,
            risk: RiskLevel::Low,
            requires_restart: false,
            applicable_modes: vec![],
            enum_values: vec![],
            description: String::new(),
        }
    }

    #[test]
    fn signed_values_round_trip() {
        let definition = parameter(RawType::I16, 1, -300.0, 300.0);
        let raw = encode_value(&definition, -12.3).unwrap();
        assert_eq!(decode_value(RawType::I16, 1, raw), -12.3);
    }

    #[test]
    fn rejects_unrepresentable_precision() {
        let definition = parameter(RawType::I16, 1, -300.0, 300.0);
        assert!(matches!(
            encode_value(&definition, 1.23),
            Err(ProfileError::Precision { .. })
        ));
    }

    #[test]
    fn parameter_id_does_not_constrain_register_address() {
        let mut definition = parameter(RawType::U16, 0, 0.0, 100.0);
        definition.parameter_id = "gain-01".into();
        definition.address = 0x2345;
        assert_eq!(definition.parameter_id, "gain-01");
        assert_eq!(definition.address, 0x2345);
    }

    #[test]
    fn generic_example_profile_is_valid() {
        let json = include_str!("../../examples/servo-profile.example.json");
        let profile = ServoProfile::from_json(json).unwrap();
        assert_eq!(profile.device.id, "example-servo");
        assert_eq!(profile.parameters.len(), 2);
        assert_eq!(profile.statuses.len(), 2);
        assert!(profile.operations.as_ref().unwrap().apply.is_some());
        assert!(profile.operations.as_ref().unwrap().persist.is_some());
    }

    #[test]
    fn operations_are_optional() {
        let json = include_str!("../../examples/servo-profile.example.json");
        let mut value: serde_json::Value = serde_json::from_str(json).unwrap();
        value.as_object_mut().unwrap().remove("operations");
        let profile = ServoProfile::from_json(&value.to_string()).unwrap();
        assert!(profile.operations.is_none());
    }
}
