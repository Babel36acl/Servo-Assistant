use crate::audit::now_ms;
use serde::{Deserialize, Serialize};
use serialport::{ClearBuffer, SerialPort};
use std::io::Write;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunicationSettings {
    pub retries: u8,
    pub max_registers: u16,
}

impl Default for CommunicationSettings {
    fn default() -> Self {
        Self {
            retries: 2,
            max_registers: 16,
        }
    }
}

impl CommunicationSettings {
    pub fn validate(&self) -> Result<(), ModbusError> {
        if self.retries > 3 || !(1..=100).contains(&self.max_registers) {
            return Err(ModbusError::InvalidResponse(
                "重试次数须为 0..3，分组上限须为 1..100".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunicationStats {
    pub transactions: u64,
    pub first_successes: u64,
    pub recovered: u64,
    pub failed: u64,
    pub crc_errors: u64,
    pub timeouts: u64,
    pub retries: u64,
    pub last_success_ms: Option<u64>,
    pub last_failure: Option<String>,
}

fn retryable(error: &ModbusError) -> bool {
    matches!(error, ModbusError::Crc)
        || matches!(error, ModbusError::Io(e) if e.kind() == std::io::ErrorKind::TimedOut)
}

fn read_captured(
    port: &mut dyn SerialPort,
    buffer: &mut [u8],
    received: &mut Vec<u8>,
) -> Result<(), ModbusError> {
    let mut offset = 0;
    while offset < buffer.len() {
        match port.read(&mut buffer[offset..]) {
            Ok(0) => return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof).into()),
            Ok(count) => {
                received.extend_from_slice(&buffer[offset..offset + count]);
                offset += count;
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum ModbusError {
    #[error("串口 I/O 错误：{0}")]
    Io(#[from] std::io::Error),
    #[error("串口错误：{0}")]
    Serial(#[from] serialport::Error),
    #[error("Modbus 响应站号不一致：期望 {expected}，实际 {actual}")]
    SlaveMismatch { expected: u8, actual: u8 },
    #[error("Modbus 响应功能码错误：期望 0x{expected:02X}，实际 0x{actual:02X}")]
    FunctionMismatch { expected: u8, actual: u8 },
    #[error("Modbus 异常响应：功能码 0x{function:02X}，异常码 0x{code:02X}")]
    Exception { function: u8, code: u8 },
    #[error("Modbus CRC 校验失败")]
    Crc,
    #[error("Modbus 响应格式无效：{0}")]
    InvalidResponse(String),
    #[error("{0}")]
    ReadFailed(String),
}

pub struct RtuClient {
    port: Box<dyn SerialPort>,
    slave_id: u8,
    inter_frame_delay: Duration,
    last_frame_end: Option<Instant>,
    pub settings: CommunicationSettings,
    pub stats: CommunicationStats,
    pub events: Vec<(String, String)>,
    received: Vec<u8>,
}

impl RtuClient {
    pub fn set_slave_id(&mut self, slave: u8) {
        self.slave_id = slave;
    }

    pub fn detect_slave(&mut self, slave: u8, address: u16) -> Result<bool, ModbusError> {
        if !(1..=247).contains(&slave) {
            return Err(ModbusError::InvalidResponse("站号必须为 1..247".into()));
        }
        self.set_slave_id(slave);
        // Probe without retries or normal-operation statistics. Two valid responses are required.
        for _ in 0..2 {
            match self.read_once(address, 1) {
                Ok(_) => {}
                Err(ModbusError::Io(error))
                    if error.kind() != std::io::ErrorKind::TimedOut
                        && error.kind() != std::io::ErrorKind::UnexpectedEof =>
                {
                    return Err(error.into())
                }
                Err(error @ ModbusError::Serial(_)) => return Err(error),
                Err(_) => return Ok(false),
            }
        }
        Ok(true)
    }
    pub fn new(
        port: Box<dyn SerialPort>,
        slave_id: u8,
        baud_rate: u32,
        bits_per_char: u32,
    ) -> Self {
        let micros = ((4_f64 * bits_per_char as f64 * 1_000_000_f64) / baud_rate as f64).ceil();
        Self {
            port,
            slave_id,
            inter_frame_delay: Duration::from_micros(micros.max(1.0) as u64),
            last_frame_end: None,
            settings: CommunicationSettings::default(),
            stats: CommunicationStats::default(),
            events: Vec::new(),
            received: Vec::new(),
        }
    }

    pub fn read_holding_registers(
        &mut self,
        address: u16,
        count: u16,
    ) -> Result<Vec<u16>, ModbusError> {
        if count == 0 || count > 100 || address.checked_add(count - 1).is_none() {
            return Err(ModbusError::InvalidResponse("读取范围无效".into()));
        }
        let mut values = Vec::new();
        let mut offset = 0;
        while offset < count {
            let size = (count - offset).min(self.settings.max_registers);
            values.extend(self.read_transaction(address + offset, size)?);
            offset += size;
        }
        Ok(values)
    }

    // One physical FC03 transaction, also used by the short/long frame comparison.
    pub fn read_transaction(&mut self, address: u16, count: u16) -> Result<Vec<u16>, ModbusError> {
        self.stats.transactions += 1;
        for attempt in 0..=self.settings.retries {
            match self.read_once(address, count) {
                Ok(values) => {
                    if attempt == 0 {
                        self.stats.first_successes += 1;
                    } else {
                        self.stats.recovered += 1;
                    }
                    self.stats.last_success_ms = Some(now_ms());
                    self.events.push((
                        "success".into(),
                        format!(
                            "FC03 address=0x{address:04X} count={count} attempts={}",
                            attempt + 1
                        ),
                    ));
                    return Ok(values);
                }
                Err(error) => {
                    if matches!(error, ModbusError::Crc) {
                        self.stats.crc_errors += 1;
                    }
                    if matches!(&error, ModbusError::Io(e) if e.kind() == std::io::ErrorKind::TimedOut)
                    {
                        self.stats.timeouts += 1;
                    }
                    let detail = format!("FC03 address=0x{address:04X} count={count} attempt={} time={} error={error} rx={:02X?}", attempt + 1, now_ms(), self.received);
                    self.stats.last_failure = Some(detail.clone());
                    if attempt < self.settings.retries && retryable(&error) {
                        self.stats.retries += 1;
                        self.events.push(("retry".into(), detail));
                    } else {
                        self.stats.failed += 1;
                        self.events.push(("failed".into(), detail.clone()));
                        return Err(ModbusError::ReadFailed(detail));
                    }
                }
            }
        }
        unreachable!()
    }

    fn read_once(&mut self, address: u16, count: u16) -> Result<Vec<u16>, ModbusError> {
        if count == 0 || count > 100 {
            return Err(ModbusError::InvalidResponse("读取数量必须为 1..100".into()));
        }
        let request = request_frame(self.slave_id, 0x03, address, count);
        self.exchange(&request, 0x03, |port, prefix, received| {
            let mut byte_count = [0_u8; 1];
            read_captured(port, &mut byte_count, received)?;
            let byte_count = byte_count[0] as usize;
            if byte_count != count as usize * 2 {
                return Err(ModbusError::InvalidResponse(format!(
                    "FC03 字节数应为 {}，实际为 {byte_count}",
                    count * 2
                )));
            }
            let mut tail = vec![0_u8; byte_count + 2];
            read_captured(port, &mut tail, received)?;
            let mut response = Vec::with_capacity(3 + tail.len());
            response.extend_from_slice(prefix);
            response.push(byte_count as u8);
            response.extend_from_slice(&tail);
            verify_crc(&response)?;
            Ok(response[3..3 + byte_count]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|bytes| u16::from_be_bytes(*bytes))
                .collect())
        })
    }

    pub fn write_single_register(&mut self, address: u16, value: u16) -> Result<(), ModbusError> {
        let request = request_frame(self.slave_id, 0x06, address, value);
        self.exchange(&request, 0x06, |port, prefix, received| {
            let mut tail = [0_u8; 6];
            read_captured(port, &mut tail, received)?;
            let mut response = Vec::with_capacity(8);
            response.extend_from_slice(prefix);
            response.extend_from_slice(&tail);
            verify_crc(&response)?;
            if response != request {
                return Err(ModbusError::InvalidResponse(
                    "FC06 回显内容与请求不一致".into(),
                ));
            }
            Ok(())
        })
    }

    fn exchange<T>(
        &mut self,
        request: &[u8],
        function: u8,
        read_success: impl FnOnce(&mut dyn SerialPort, &[u8; 2], &mut Vec<u8>) -> Result<T, ModbusError>,
    ) -> Result<T, ModbusError> {
        self.wait_for_inter_frame_gap();
        self.received.clear();
        let result = (|| {
            self.port.clear(ClearBuffer::Input)?;
            self.port.write_all(request)?;
            self.port.flush()?;

            let mut prefix = [0_u8; 2];
            read_captured(self.port.as_mut(), &mut prefix, &mut self.received)?;
            if prefix[0] != self.slave_id {
                return Err(ModbusError::SlaveMismatch {
                    expected: self.slave_id,
                    actual: prefix[0],
                });
            }
            if prefix[1] == function | 0x80 {
                let mut tail = [0_u8; 3];
                read_captured(self.port.as_mut(), &mut tail, &mut self.received)?;
                let response = [prefix[0], prefix[1], tail[0], tail[1], tail[2]];
                verify_crc(&response)?;
                return Err(ModbusError::Exception {
                    function,
                    code: tail[0],
                });
            }
            if prefix[1] != function {
                return Err(ModbusError::FunctionMismatch {
                    expected: function,
                    actual: prefix[1],
                });
            }
            read_success(self.port.as_mut(), &prefix, &mut self.received)
        })();
        self.last_frame_end = Some(Instant::now());
        result
    }

    fn wait_for_inter_frame_gap(&self) {
        if let Some(last_frame_end) = self.last_frame_end {
            let elapsed = last_frame_end.elapsed();
            if elapsed < self.inter_frame_delay {
                std::thread::sleep(self.inter_frame_delay - elapsed);
            }
        }
    }
}

fn request_frame(slave_id: u8, function: u8, address: u16, value: u16) -> Vec<u8> {
    let mut frame = vec![slave_id, function];
    frame.extend_from_slice(&address.to_be_bytes());
    frame.extend_from_slice(&value.to_be_bytes());
    let crc = modbus_crc(&frame);
    frame.extend_from_slice(&crc.to_le_bytes());
    frame
}

fn verify_crc(frame: &[u8]) -> Result<(), ModbusError> {
    if frame.len() < 4 {
        return Err(ModbusError::InvalidResponse("响应过短".into()));
    }
    let payload_len = frame.len() - 2;
    let expected = modbus_crc(&frame[..payload_len]);
    let actual = u16::from_le_bytes([frame[payload_len], frame[payload_len + 1]]);
    if expected != actual {
        return Err(ModbusError::Crc);
    }
    Ok(())
}

pub fn modbus_crc(bytes: &[u8]) -> u16 {
    let mut crc = 0xFFFF_u16;
    for byte in bytes {
        crc ^= *byte as u16;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xA001
            } else {
                crc >> 1
            };
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_known_fc03_frame() {
        assert_eq!(modbus_crc(&[0x01, 0x03, 0x00, 0x05, 0x00, 0x02]), 0x0AD4);
    }
}

#[cfg(test)]
#[path = "modbus_tests.rs"]
mod transport_tests;
