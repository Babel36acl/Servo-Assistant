//! 模拟器会话：按 Profile 建立寄存器/EEPROM 快照，并模拟应用/持久化状态机。
use super::RuntimeError;
use crate::profile::{encode_value, OperationSet, ServoProfile};
use std::collections::{HashMap, HashSet};

pub(super) struct SimulatorDevice {
    registers: HashMap<u16, u16>,
    eeprom: HashMap<u16, u16>,
    parameter_addresses: HashSet<u16>,
    operations: Option<OperationSet>,
    pub(super) recorder: Option<crate::recording::Recorder>,
}

impl SimulatorDevice {
    pub(super) fn new(profile: &ServoProfile) -> Result<Self, RuntimeError> {
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

    pub(super) fn read_registers(&self, address: u16, count: u16) -> Vec<u16> {
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

    pub(super) fn write_register(&mut self, address: u16, value: u16) {
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
