use super::*;
use serialport::{DataBits, FlowControl, Parity, StopBits};
use std::collections::VecDeque;
use std::io::{self, Read};
use std::sync::{Arc, Mutex};

// Script response bytes per request; exercise the actual RTU parser and retry loop.
struct ScriptPort {
    responses: VecDeque<Vec<u8>>,
    current: VecDeque<u8>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}
impl Read for ScriptPort {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.current.is_empty() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        // Deliberately fragment responses to exercise partial reads.
        buffer[0] = self.current.pop_front().unwrap();
        Ok(1)
    }
}
impl Write for ScriptPort {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes.lock().unwrap().push(bytes.to_vec());
        self.current = self.responses.pop_front().unwrap_or_default().into();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl SerialPort for ScriptPort {
    fn name(&self) -> Option<String> {
        None
    }
    fn baud_rate(&self) -> serialport::Result<u32> {
        Ok(19200)
    }
    fn data_bits(&self) -> serialport::Result<DataBits> {
        Ok(DataBits::Eight)
    }
    fn flow_control(&self) -> serialport::Result<FlowControl> {
        Ok(FlowControl::None)
    }
    fn parity(&self) -> serialport::Result<Parity> {
        Ok(Parity::Even)
    }
    fn stop_bits(&self) -> serialport::Result<StopBits> {
        Ok(StopBits::One)
    }
    fn timeout(&self) -> Duration {
        Duration::from_millis(800)
    }
    fn set_baud_rate(&mut self, _: u32) -> serialport::Result<()> {
        Ok(())
    }
    fn set_data_bits(&mut self, _: DataBits) -> serialport::Result<()> {
        Ok(())
    }
    fn set_flow_control(&mut self, _: FlowControl) -> serialport::Result<()> {
        Ok(())
    }
    fn set_parity(&mut self, _: Parity) -> serialport::Result<()> {
        Ok(())
    }
    fn set_stop_bits(&mut self, _: StopBits) -> serialport::Result<()> {
        Ok(())
    }
    fn set_timeout(&mut self, _: Duration) -> serialport::Result<()> {
        Ok(())
    }
    fn write_request_to_send(&mut self, _: bool) -> serialport::Result<()> {
        Ok(())
    }
    fn write_data_terminal_ready(&mut self, _: bool) -> serialport::Result<()> {
        Ok(())
    }
    fn read_clear_to_send(&mut self) -> serialport::Result<bool> {
        Ok(true)
    }
    fn read_data_set_ready(&mut self) -> serialport::Result<bool> {
        Ok(true)
    }
    fn read_ring_indicator(&mut self) -> serialport::Result<bool> {
        Ok(false)
    }
    fn read_carrier_detect(&mut self) -> serialport::Result<bool> {
        Ok(true)
    }
    fn bytes_to_read(&self) -> serialport::Result<u32> {
        Ok(self.current.len() as u32)
    }
    fn bytes_to_write(&self) -> serialport::Result<u32> {
        Ok(0)
    }
    fn clear(&self, _: ClearBuffer) -> serialport::Result<()> {
        Ok(())
    }
    fn try_clone(&self) -> serialport::Result<Box<dyn SerialPort>> {
        unimplemented!()
    }
    fn set_break(&self) -> serialport::Result<()> {
        Ok(())
    }
    fn clear_break(&self) -> serialport::Result<()> {
        Ok(())
    }
}
fn frame(mut bytes: Vec<u8>) -> Vec<u8> {
    bytes.extend_from_slice(&modbus_crc(&bytes).to_le_bytes());
    bytes
}
fn client(responses: Vec<Vec<u8>>) -> RtuClient {
    RtuClient::new(
        Box::new(ScriptPort {
            responses: responses.into(),
            current: VecDeque::new(),
            writes: Arc::default(),
        }),
        1,
        19200,
        11,
    )
}

#[test]
fn crc_failure_recovers_with_original_failure_evidence() {
    let good = frame(vec![1, 3, 2, 0, 42]);
    let mut bad = good.clone();
    bad[4] ^= 1;
    let mut client = client(vec![bad, good]);
    assert_eq!(client.read_holding_registers(0x1000, 1).unwrap(), vec![42]);
    assert_eq!(
        (
            client.stats.crc_errors,
            client.stats.retries,
            client.stats.recovered,
            client.stats.first_successes
        ),
        (1, 1, 1, 0)
    );
    assert!(client
        .stats
        .last_failure
        .unwrap()
        .contains("address=0x1000 count=1 attempt=1"));
    assert_eq!(client.events[0].0, "retry");
}

#[test]
fn timeout_retries_are_bounded_and_capture_partial_frame() {
    let mut client = client(vec![vec![1, 3], vec![], vec![1]]);
    let error = client
        .read_holding_registers(12, 1)
        .unwrap_err()
        .to_string();
    assert_eq!(
        (
            client.stats.timeouts,
            client.stats.retries,
            client.stats.failed
        ),
        (3, 2, 1)
    );
    assert!(error.contains("attempt=3"));
    assert!(error.contains("rx=[01]"));
    assert_eq!(client.events.len(), 3);
}

#[test]
fn device_exception_is_not_retried() {
    let mut client = client(vec![frame(vec![1, 0x83, 2])]);
    assert!(client.read_holding_registers(12, 1).is_err());
    assert_eq!((client.stats.retries, client.stats.failed), (0, 1));
}

#[test]
fn write_is_never_retried() {
    let writes = Arc::default();
    let port = ScriptPort {
        responses: VecDeque::new(),
        current: VecDeque::new(),
        writes: Arc::clone(&writes),
    };
    let mut client = RtuClient::new(Box::new(port), 1, 19200, 11);
    assert!(client.write_single_register(12, 42).is_err());
    assert_eq!(writes.lock().unwrap().len(), 1);
}

#[test]
fn configured_reads_split_but_probe_preserves_long_frame() {
    let mut client = client(vec![
        frame(vec![1, 3, 2, 0, 1]),
        frame(vec![1, 3, 2, 0, 2]),
        frame(vec![1, 3, 4, 0, 3, 0, 4]),
    ]);
    client.settings.max_registers = 1;
    assert_eq!(client.read_holding_registers(12, 2).unwrap(), vec![1, 2]);
    assert_eq!(client.read_transaction(12, 2).unwrap(), vec![3, 4]);
    assert_eq!(client.stats.transactions, 3);
}

#[test]
fn zero_retries_and_settings_limits() {
    let mut client = client(vec![]);
    client.settings.retries = 0;
    assert!(client.read_holding_registers(12, 1).is_err());
    assert_eq!(client.stats.timeouts, 1);
    assert!(CommunicationSettings {
        retries: 4,
        max_registers: 16
    }
    .validate()
    .is_err());
    assert!(CommunicationSettings {
        retries: 2,
        max_registers: 0
    }
    .validate()
    .is_err());
    assert!(client.read_holding_registers(u16::MAX, 2).is_err());
}
