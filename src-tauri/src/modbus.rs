use serialport::{ClearBuffer, SerialPort};
use std::io::{Read, Write};
use std::time::{Duration, Instant};
use thiserror::Error;

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
}

pub struct RtuClient {
    port: Box<dyn SerialPort>,
    slave_id: u8,
    inter_frame_delay: Duration,
    last_frame_end: Option<Instant>,
}

impl RtuClient {
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
        }
    }

    pub fn read_holding_registers(
        &mut self,
        address: u16,
        count: u16,
    ) -> Result<Vec<u16>, ModbusError> {
        if count == 0 || count > 100 {
            return Err(ModbusError::InvalidResponse("读取数量必须为 1..100".into()));
        }
        let request = request_frame(self.slave_id, 0x03, address, count);
        self.exchange(&request, 0x03, |port, prefix| {
            let mut byte_count = [0_u8; 1];
            port.read_exact(&mut byte_count)?;
            let byte_count = byte_count[0] as usize;
            if byte_count != count as usize * 2 {
                return Err(ModbusError::InvalidResponse(format!(
                    "FC03 字节数应为 {}，实际为 {byte_count}",
                    count * 2
                )));
            }
            let mut tail = vec![0_u8; byte_count + 2];
            port.read_exact(&mut tail)?;
            let mut response = Vec::with_capacity(3 + tail.len());
            response.extend_from_slice(prefix);
            response.push(byte_count as u8);
            response.extend_from_slice(&tail);
            verify_crc(&response)?;
            Ok(response[3..3 + byte_count]
                .chunks_exact(2)
                .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
                .collect())
        })
    }

    pub fn write_single_register(&mut self, address: u16, value: u16) -> Result<(), ModbusError> {
        let request = request_frame(self.slave_id, 0x06, address, value);
        self.exchange(&request, 0x06, |port, prefix| {
            let mut tail = [0_u8; 6];
            port.read_exact(&mut tail)?;
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
        read_success: impl FnOnce(&mut dyn SerialPort, &[u8; 2]) -> Result<T, ModbusError>,
    ) -> Result<T, ModbusError> {
        self.wait_for_inter_frame_gap();
        let result = (|| {
            self.port.clear(ClearBuffer::Input)?;
            self.port.write_all(request)?;
            self.port.flush()?;

            let mut prefix = [0_u8; 2];
            self.port.read_exact(&mut prefix)?;
            if prefix[0] != self.slave_id {
                return Err(ModbusError::SlaveMismatch {
                    expected: self.slave_id,
                    actual: prefix[0],
                });
            }
            if prefix[1] == function | 0x80 {
                let mut tail = [0_u8; 3];
                self.port.read_exact(&mut tail)?;
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
            read_success(self.port.as_mut(), &prefix)
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
