//! Modbus TCP (MBAP) client. Only FC03/FC06 are implemented so the TCP link
//! shares the same read/write contract as the serial client.
use crate::audit::now_ms;
use crate::modbus::{CommunicationSettings, CommunicationStats, ModbusError};
use crate::recording::Recorder;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

pub struct TcpClient {
    stream: TcpStream,
    unit_id: u8,
    pub settings: CommunicationSettings,
    pub stats: CommunicationStats,
    pub events: Vec<(String, String)>,
    transaction: u16,
    source: String,
    recorder: Option<Recorder>,
}

fn request_frame(transaction: u16, unit: u8, pdu: &[u8]) -> Vec<u8> {
    let length = (pdu.len() + 1) as u16;
    let mut frame = Vec::with_capacity(7 + pdu.len());
    frame.extend_from_slice(&transaction.to_be_bytes());
    frame.extend_from_slice(&[0, 0]);
    frame.extend_from_slice(&length.to_be_bytes());
    frame.push(unit);
    frame.extend_from_slice(pdu);
    frame
}

fn fc03_pdu(address: u16, count: u16) -> Vec<u8> {
    let mut pdu = vec![0x03];
    pdu.extend_from_slice(&address.to_be_bytes());
    pdu.extend_from_slice(&count.to_be_bytes());
    pdu
}

fn fc06_pdu(address: u16, value: u16) -> Vec<u8> {
    let mut pdu = vec![0x06];
    pdu.extend_from_slice(&address.to_be_bytes());
    pdu.extend_from_slice(&value.to_be_bytes());
    pdu
}

fn retryable(error: &ModbusError) -> bool {
    matches!(
        error,
        ModbusError::Io(io)
            if matches!(
                io.kind(),
                std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::WouldBlock
                    | std::io::ErrorKind::UnexpectedEof
            )
    )
}

impl TcpClient {
    pub fn new(
        stream: TcpStream,
        unit_id: u8,
        timeout: Duration,
        source: String,
        recorder: Option<Recorder>,
    ) -> Self {
        let _ = stream.set_read_timeout(Some(timeout));
        let _ = stream.set_write_timeout(Some(timeout));
        let _ = stream.set_nodelay(true);
        Self {
            stream,
            unit_id,
            settings: CommunicationSettings::default(),
            stats: CommunicationStats::default(),
            events: Vec::new(),
            transaction: 0,
            source,
            recorder,
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

    pub fn read_transaction(&mut self, address: u16, count: u16) -> Result<Vec<u16>, ModbusError> {
        if count == 0 || count > 100 {
            return Err(ModbusError::InvalidResponse("读取数量必须为 1..100".into()));
        }
        self.stats.transactions += 1;
        for attempt in 0..=self.settings.retries {
            match self.exchange(&fc03_pdu(address, count)) {
                Ok(pdu) => {
                    let expected = count as usize * 2;
                    if pdu.len() != expected + 2 || pdu[0] != 0x03 {
                        return Err(ModbusError::InvalidResponse(
                            "FC03 响应长度或功能码不一致".into(),
                        ));
                    }
                    if pdu[1] as usize != expected {
                        return Err(ModbusError::InvalidResponse(
                            "FC03 字节数与请求不一致".into(),
                        ));
                    }
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
                    return Ok(pdu[2..]
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|bytes| u16::from_be_bytes(*bytes))
                        .collect());
                }
                Err(error) => {
                    let detail = format!(
                        "FC03 address=0x{address:04X} count={count} attempt={} error={error}",
                        attempt + 1
                    );
                    if matches!(&error, ModbusError::Io(io) if matches!(io.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock))
                    {
                        self.stats.timeouts += 1;
                    }
                    self.stats.last_failure = Some(detail.clone());
                    if attempt < self.settings.retries && retryable(&error) {
                        self.drain();
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

    pub fn write_single_register(&mut self, address: u16, value: u16) -> Result<(), ModbusError> {
        self.stats.transactions += 1;
        let pdu = fc06_pdu(address, value);
        match self.exchange(&pdu) {
            Ok(response) => {
                if response.len() != 5 || response[0] != 0x06 || response[1..] != pdu[1..] {
                    let detail =
                        format!("FC06 address=0x{address:04X} value={value} response mismatch");
                    self.stats.failed += 1;
                    self.stats.last_failure = Some(detail.clone());
                    self.events.push(("failed".into(), detail));
                    return Err(ModbusError::InvalidResponse("FC06 写入回显不一致".into()));
                }
                self.stats.first_successes += 1;
                self.stats.last_success_ms = Some(now_ms());
                self.events.push((
                    "success".into(),
                    format!("FC06 address=0x{address:04X} value={value}"),
                ));
                Ok(())
            }
            Err(error) => {
                let detail = format!("FC06 address=0x{address:04X} value={value} error={error}");
                self.stats.failed += 1;
                self.stats.last_failure = Some(detail.clone());
                self.events.push(("failed".into(), detail));
                Err(error)
            }
        }
    }

    fn exchange(&mut self, pdu: &[u8]) -> Result<Vec<u8>, ModbusError> {
        self.transaction = self.transaction.wrapping_add(1);
        let frame = request_frame(self.transaction, self.unit_id, pdu);
        self.record("tx", &frame, "Modbus TCP request");
        self.stream.write_all(&frame)?;
        self.stream.flush()?;
        let mut header = [0u8; 7];
        self.stream.read_exact(&mut header)?;
        self.record("rx", &header, "Modbus TCP MBAP header");
        let transaction = u16::from_be_bytes([header[0], header[1]]);
        let protocol = u16::from_be_bytes([header[2], header[3]]);
        let length = u16::from_be_bytes([header[4], header[5]]) as usize;
        let unit = header[6];
        if protocol != 0 {
            return Err(ModbusError::InvalidResponse("MBAP 协议标识不为 0".into()));
        }
        if transaction != self.transaction {
            return Err(ModbusError::InvalidResponse("MBAP 事务号不匹配".into()));
        }
        if unit != self.unit_id {
            return Err(ModbusError::SlaveMismatch {
                expected: self.unit_id,
                actual: unit,
            });
        }
        if !(3..=260).contains(&length) {
            return Err(ModbusError::InvalidResponse("MBAP 长度无效".into()));
        }
        let mut response = vec![0u8; length - 1];
        self.stream.read_exact(&mut response)?;
        self.record("rx", &response, "Modbus TCP PDU");
        if response.len() < 2 {
            return Err(ModbusError::InvalidResponse("Modbus TCP 响应过短".into()));
        }
        if response[0] & 0x80 != 0 {
            return Err(ModbusError::Exception {
                function: response[0] & 0x7f,
                code: response[1],
            });
        }
        Ok(response)
    }

    fn drain(&mut self) {
        // Drop any partial bytes left by a timed-out transaction before retrying.
        if self.stream.set_nonblocking(true).is_err() {
            return;
        }
        let mut buffer = [0u8; 256];
        while self.stream.read(&mut buffer).is_ok() {}
        let _ = self.stream.set_nonblocking(false);
    }

    fn record(&self, direction: &str, bytes: &[u8], detail: &str) {
        let Some(recorder) = &self.recorder else {
            return;
        };
        recorder.emit(
            &self.source,
            "modbus-tcp",
            direction,
            self.transaction as u64,
            bytes,
            detail,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    fn serve_once(
        response: impl FnOnce(&[u8]) -> Vec<u8> + Send + 'static,
    ) -> (std::net::SocketAddr, thread::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut header = [0u8; 7];
            socket.read_exact(&mut header).unwrap();
            let length = u16::from_be_bytes([header[4], header[5]]) as usize - 1;
            let mut pdu = vec![0u8; length];
            socket.read_exact(&mut pdu).unwrap();
            let mut request = header.to_vec();
            request.extend_from_slice(&pdu);
            let transaction = u16::from_be_bytes([header[0], header[1]]);
            socket
                .write_all(&response_with(transaction, 1, &response(&request)))
                .unwrap();
            request
        });
        (address, handle)
    }

    fn response_with(transaction: u16, unit: u8, pdu: &[u8]) -> Vec<u8> {
        let mut frame = Vec::new();
        frame.extend_from_slice(&transaction.to_be_bytes());
        frame.extend_from_slice(&[0, 0]);
        frame.extend_from_slice(&((pdu.len() + 1) as u16).to_be_bytes());
        frame.push(unit);
        frame.extend_from_slice(pdu);
        frame
    }

    fn connect_client(address: std::net::SocketAddr) -> TcpClient {
        TcpClient::new(
            TcpStream::connect(address).unwrap(),
            1,
            Duration::from_millis(500),
            "test".into(),
            None,
        )
    }

    #[test]
    fn mbap_header_is_big_endian_and_bounded() {
        let frame = request_frame(0x1234, 7, &fc03_pdu(0x0102, 3));
        assert_eq!(frame[..7], [0x12, 0x34, 0, 0, 0, 6, 7]);
        assert_eq!(&frame[7..], &[0x03, 0x01, 0x02, 0x00, 0x03]);
    }

    #[test]
    fn read_and_write_round_trip() {
        let (address, handle) = serve_once(|request| {
            assert_eq!(request[7], 0x03);
            vec![0x03, 4, 0, 10, 0, 20]
        });
        let mut client = connect_client(address);
        assert_eq!(client.read_transaction(1, 2).unwrap(), vec![10, 20]);
        handle.join().unwrap();

        let (address, handle) = serve_once(|request| request[7..].to_vec());
        let mut client = connect_client(address);
        client.write_single_register(1, 42).unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn exception_and_mismatched_transaction_are_not_retried() {
        let (address, handle) = serve_once(|_| vec![0x83, 0x02]);
        let mut client = connect_client(address);
        let error = client.read_transaction(1, 1).unwrap_err();
        assert!(error.to_string().contains("异常码 0x02"), "{error}");
        assert_eq!(client.settings.retries, 2);
        handle.join().unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0u8; 12];
            socket.read_exact(&mut request).unwrap();
            socket
                .write_all(&response_with(0xFFFF, 1, &[0x03, 2, 0, 1]))
                .unwrap();
        });
        let mut client = connect_client(address);
        assert!(client.read_transaction(1, 1).is_err());
        handle.join().unwrap();
    }
}
