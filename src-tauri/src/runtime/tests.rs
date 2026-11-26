//! 运行时单元测试：连接参数校验、模拟器状态机、分组读取与批量写前校验。
use super::params::{
    preflight_planned_writes, read_parameter_definitions, read_status_definitions,
    write_planned_parameter,
};
use super::simulator::SimulatorDevice;
use super::types::{ConnectionPreset, ConnectionRequest};
use super::{RuntimeError, Session};
use crate::modbus::Protocol;
use crate::profile::{
    Access, DeviceInfo, OperationDefinition, OperationSet, ParameterDefinition, ParitySetting,
    RawType, RiskLevel, ServoProfile, StatusDefinition, TransportProfile,
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

#[test]
fn tcp_endpoint_validation_ignores_serial_fields() {
    let profile = test_profile();
    let mut request: ConnectionRequest = serde_json::from_value(serde_json::json!({
        "mode": "tcp",
        "host": "127.0.0.1",
        "port": 502,
        "slaveId": 1,
        "baudRate": 9600,
        "parity": "even",
        "stopBits": 1,
        "timeoutMs": 200
    }))
    .unwrap();
    // TCP does not use the Profile baud list or serial framing fields.
    assert!(request.validate(&profile).is_ok());
    request.host = Some("  ".into());
    assert!(request.validate(&profile).is_err());
    request.host = Some("127.0.0.1".into());
    request.port = Some(0);
    assert!(request.validate(&profile).is_err());
    request.port = Some(502);
    request.slave_id = 0;
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
