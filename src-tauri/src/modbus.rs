use crate::audit::now_ms;
use serde::{Deserialize, Serialize};
use serialport::{ClearBuffer, SerialPort};
use std::io::Write;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    #[default]
    Rtu,
    Ascii,
}
impl Protocol {
    pub fn label(self) -> &'static str {
        match self {
            Self::Rtu => "modbus-rtu",
            Self::Ascii => "modbus-ascii",
        }
    }
}

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
    pub lrc_errors: u64,
    pub timeouts: u64,
    pub retries: u64,
    pub last_success_ms: Option<u64>,
    pub last_failure: Option<String>,
}

fn retryable(error: &ModbusError) -> bool {
    matches!(error, ModbusError::Crc | ModbusError::Lrc)
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
    #[error("Modbus LRC 校验失败")]
    Lrc,
    #[error("Modbus 响应格式无效：{0}")]
    InvalidResponse(String),
    #[error("{0}")]
    ReadFailed(String),
}

pub struct SerialClient {
    port: Box<dyn SerialPort>,
    slave_id: u8,
    protocol: Protocol,
    inter_frame_delay: Duration,
    last_frame_end: Option<Instant>,
    pub settings: CommunicationSettings,
    pub stats: CommunicationStats,
    pub events: Vec<(String, String)>,
    received: Vec<u8>,
    capture_transaction: std::sync::Arc<std::sync::atomic::AtomicU64>,
    capture: Option<(crate::recording::Recorder, String)>,
}

impl SerialClient {
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
            protocol: Protocol::Rtu,
            inter_frame_delay: Duration::from_micros(micros.max(1.0) as u64),
            last_frame_end: None,
            settings: CommunicationSettings::default(),
            stats: CommunicationStats::default(),
            events: Vec::new(),
            received: Vec::new(),
            capture_transaction: Default::default(),
            capture: None,
        }
    }

    pub fn with_protocol(mut self, protocol: Protocol) -> Self {
        self.protocol = protocol;
        self
    }

    pub fn with_recorder(mut self, recorder: crate::recording::Recorder) -> Self {
        let source = format!(
            "serial:{} baud={:?} parity={:?} connection={}",
            self.port.name().unwrap_or_default(),
            self.port.baud_rate(),
            self.port.parity(),
            crate::recording::timestamp_us()
        );
        self.capture = Some((recorder.clone(), source.clone()));
        self.port = Box::new(crate::recorded_port::RecordedPort {
            port: self.port,
            recorder,
            transaction: self.capture_transaction.clone(),
            source,
            protocol: self.protocol.label(),
        });
        self
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
                    if matches!(error, ModbusError::Lrc) {
                        self.stats.lrc_errors += 1;
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
        let request = request_payload(self.slave_id, 0x03, address, count);
        let response = self.exchange(&request, 0x03)?;
        Ok(response[3..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|bytes| u16::from_be_bytes(*bytes))
            .collect())
    }
    pub fn write_single_register(&mut self, address: u16, value: u16) -> Result<(), ModbusError> {
        let request = request_payload(self.slave_id, 0x06, address, value);
        self.exchange(&request, 0x06)?;
        Ok(())
    }
    fn exchange(&mut self, payload: &[u8], function: u8) -> Result<Vec<u8>, ModbusError> {
        self.capture_transaction
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.wait_for_inter_frame_gap();
        self.received.clear();
        let request = match self.protocol {
            Protocol::Ascii => ascii_frame(payload),
            Protocol::Rtu => {
                let mut bytes = payload.to_vec();
                bytes.extend_from_slice(&modbus_crc(payload).to_le_bytes());
                bytes
            }
        };
        let timeout = self.port.timeout();
        let result = (|| {
            self.port.clear(ClearBuffer::Input)?;
            self.port.write_all(&request)?;
            self.port.flush()?;
            let response = match self.protocol {
                Protocol::Ascii => {
                    // Bound the entire frame so noise cannot indefinitely extend a cancelled scan.
                    let deadline = Instant::now() + timeout;
                    loop {
                        let remaining = deadline.saturating_duration_since(Instant::now());
                        if remaining.is_zero() {
                            return Err(std::io::Error::from(std::io::ErrorKind::TimedOut).into());
                        }
                        self.port.set_timeout(remaining)?;
                        let mut byte = [0];
                        read_captured(self.port.as_mut(), &mut byte, &mut self.received)?;
                        if self.received.len() == 1 && byte[0] != b':' {
                            return Err(ModbusError::InvalidResponse("ASCII 帧缺少冒号".into()));
                        }
                        if byte[0] == b'\n' {
                            break;
                        }
                        if self.received.len() >= 513 {
                            return Err(ModbusError::InvalidResponse("ASCII 帧过长".into()));
                        }
                    }
                    decode_ascii(&self.received)?
                }
                Protocol::Rtu => {
                    let mut prefix = [0; 2];
                    read_captured(self.port.as_mut(), &mut prefix, &mut self.received)?;
                    if prefix[0] != self.slave_id {
                        return Err(ModbusError::SlaveMismatch {
                            expected: self.slave_id,
                            actual: prefix[0],
                        });
                    }
                    if prefix[1] != function && prefix[1] != function | 0x80 {
                        return Err(ModbusError::FunctionMismatch {
                            expected: function,
                            actual: prefix[1],
                        });
                    }
                    let tail_len = if prefix[1] == function | 0x80 {
                        3
                    } else if prefix[1] == 0x03 {
                        let mut count = [0];
                        read_captured(self.port.as_mut(), &mut count, &mut self.received)?;
                        if count[0] as u16 != u16::from_be_bytes([payload[4], payload[5]]) * 2 {
                            return Err(ModbusError::InvalidResponse(
                                "FC03 字节数与请求不一致".into(),
                            ));
                        }
                        count[0] as usize + 2
                    } else if prefix[1] == 0x06 {
                        6
                    } else {
                        return Err(ModbusError::FunctionMismatch {
                            expected: function,
                            actual: prefix[1],
                        });
                    };
                    let mut tail = vec![0; tail_len];
                    read_captured(self.port.as_mut(), &mut tail, &mut self.received)?;
                    verify_crc(&self.received)?;
                    self.received[..self.received.len() - 2].to_vec()
                }
            };
            if response[0] != self.slave_id {
                return Err(ModbusError::SlaveMismatch {
                    expected: self.slave_id,
                    actual: response[0],
                });
            }
            if response[1] == function | 0x80 {
                if response.len() != 3 {
                    return Err(ModbusError::InvalidResponse("异常响应长度错误".into()));
                }
                return Err(ModbusError::Exception {
                    function,
                    code: response[2],
                });
            }
            if response[1] != function {
                return Err(ModbusError::FunctionMismatch {
                    expected: function,
                    actual: response[1],
                });
            }
            if function == 0x03 {
                let count = u16::from_be_bytes([payload[4], payload[5]]) as usize;
                if response.len() != 3 + count * 2 || response[2] as usize != count * 2 {
                    return Err(ModbusError::InvalidResponse(
                        "FC03 字节数与请求不一致".into(),
                    ));
                }
            } else if function == 0x06 && response != payload {
                return Err(ModbusError::InvalidResponse(
                    "FC06 回显内容与请求不一致".into(),
                ));
            }
            Ok(response)
        })();
        let restored = self.port.set_timeout(timeout).map_err(ModbusError::from);
        let result = result.and_then(|response| restored.map(|()| response));
        if let Some((recorder, source)) = &self.capture {
            recorder.emit(
                source,
                self.protocol.label(),
                "event",
                self.capture_transaction
                    .load(std::sync::atomic::Ordering::Relaxed),
                &[],
                &format!(
                    "FC{function:02X} request={request:02X?} result={:?}",
                    result
                        .as_ref()
                        .map(|_| "success")
                        .map_err(|e| e.to_string())
                ),
            );
        }
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

fn ascii_frame(payload: &[u8]) -> Vec<u8> {
    let lrc = payload
        .iter()
        .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
        .wrapping_neg();
    let mut frame = String::from(":");
    for byte in payload.iter().chain(std::iter::once(&lrc)) {
        use std::fmt::Write;
        write!(frame, "{byte:02X}").unwrap();
    }
    frame.push_str("\r\n");
    frame.into_bytes()
}
fn decode_ascii(frame: &[u8]) -> Result<Vec<u8>, ModbusError> {
    if frame.len() < 9
        || frame.len() > 513
        || frame[0] != b':'
        || !frame.ends_with(b"\r\n")
        || !(frame.len() - 3).is_multiple_of(2)
    {
        return Err(ModbusError::InvalidResponse(
            "ASCII 帧长度或结束符错误".into(),
        ));
    }
    let mut bytes = Vec::new();
    for pair in frame[1..frame.len() - 2].as_chunks::<2>().0 {
        match (
            (pair[0] as char).to_digit(16),
            (pair[1] as char).to_digit(16),
        ) {
            (Some(h), Some(l)) => bytes.push((h * 16 + l) as u8),
            _ => {
                return Err(ModbusError::InvalidResponse(
                    "ASCII 包含非十六进制字符".into(),
                ))
            }
        }
    }
    if bytes.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) != 0 {
        return Err(ModbusError::Lrc);
    }
    bytes.pop();
    Ok(bytes)
}

fn request_payload(slave_id: u8, function: u8, address: u16, value: u16) -> Vec<u8> {
    let mut frame = vec![slave_id, function];
    frame.extend_from_slice(&address.to_be_bytes());
    frame.extend_from_slice(&value.to_be_bytes());
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
