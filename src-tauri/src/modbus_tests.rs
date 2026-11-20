use super::*;
use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex};

// Script response bytes per request; exercise the actual RTU parser and retry loop.
struct ScriptPort {
    responses: VecDeque<Vec<u8>>,
    current: VecDeque<u8>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}
impl Transport for ScriptPort {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.current.is_empty() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        // Deliberately fragment responses to exercise partial reads.
        buffer[0] = self.current.pop_front().unwrap();
        Ok(1)
    }
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writes.lock().unwrap().push(bytes.to_vec());
        self.current = self.responses.pop_front().unwrap_or_default().into();
        Ok(())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn clear_input(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn set_timeout(&mut self, _: Duration) -> io::Result<()> {
        Ok(())
    }
    fn timeout(&self) -> Duration {
        Duration::from_millis(800)
    }
    fn name(&self) -> String {
        "script".into()
    }
}
fn frame(mut bytes: Vec<u8>) -> Vec<u8> {
    bytes.extend_from_slice(&modbus_crc(&bytes).to_le_bytes());
    bytes
}
fn client(responses: Vec<Vec<u8>>) -> SerialClient {
    SerialClient::new(
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
    let mut client = SerialClient::new(Box::new(port), 1, 19200, 11);
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

#[test]
fn discovery_requires_two_valid_responses_and_preserves_station_check() {
    let good = frame(vec![1, 3, 2, 0, 42]);
    let mut detected = client(vec![good.clone(), good.clone()]);
    assert!(detected.detect_slave(1, 0x1000).unwrap());
    let mut single = client(vec![good.clone()]);
    assert!(!single.detect_slave(1, 0x1000).unwrap());
    let mut wrong_station = client(vec![good]);
    assert!(!wrong_station.detect_slave(2, 0x1000).unwrap());
    assert!(wrong_station.detect_slave(0, 0x1000).is_err());
}

#[test]
fn recording_covers_partial_reads_crc_retry_and_does_not_retry_writes() {
    let root = std::env::temp_dir().join(format!(
        "servo-serial-rec-{}",
        crate::recording::timestamp_us()
    ));
    let recorder = crate::recording::Recorder::new(root.clone());
    recorder.start(16).unwrap();
    let good = frame(vec![1, 3, 2, 0, 42]);
    let mut bad = good.clone();
    bad[4] ^= 1;
    let mut c = client(vec![bad.clone(), good.clone()]).with_recorder(recorder.clone());
    assert_eq!(c.read_transaction(12, 1).unwrap(), [42]);
    assert!(c.write_single_register(12, 9).is_err());
    let status = recorder.stop().unwrap();
    let index = crate::recording_index::RecordingIndex::new(root.join("indexes"));
    let records = index
        .page(&status.path, 0, &Default::default(), false)
        .unwrap()
        .records;
    let tx = records
        .iter()
        .filter(|r| r.direction == "tx")
        .collect::<Vec<_>>();
    assert_eq!(tx.len(), 3);
    assert_eq!(tx[0].bytes[1], 3);
    assert_eq!(tx[2].bytes[1], 6);
    assert_ne!(tx[0].transaction, tx[1].transaction);
    let first = index
        .transaction(
            &status.path,
            &tx[0].source,
            &tx[0].protocol,
            tx[0].transaction,
        )
        .unwrap();
    assert!(first.response.error.unwrap().contains("CRC"));
    assert!(first.outcome.contains("失败"));
    let recovered = index
        .transaction(
            &status.path,
            &tx[1].source,
            &tx[1].protocol,
            tx[1].transaction,
        )
        .unwrap();
    assert_eq!(recovered.response.values, [42]);
    assert_eq!(recovered.outcome, "请求与响应匹配");
    let failed_write = index
        .transaction(
            &status.path,
            &tx[2].source,
            &tx[2].protocol,
            tx[2].transaction,
        )
        .unwrap();
    assert!(failed_write.outcome.contains("失败"));
    assert_eq!(failed_write.request.values, [9]);

    let received = records
        .iter()
        .filter(|r| r.direction == "rx")
        .flat_map(|r| r.bytes.clone())
        .collect::<Vec<_>>();
    assert_eq!(received, [bad, good].concat());
    assert!(records.iter().any(|r| r.detail.contains("CRC")));
    assert!(records
        .iter()
        .any(|r| r.detail.contains("discarded bytes unavailable")));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn ascii_known_wire_frame_and_fragmented_read() {
    let writes = Arc::default();
    let port = ScriptPort {
        responses: vec![b":010302002AD0\r\n".to_vec()].into(),
        current: VecDeque::new(),
        writes: Arc::clone(&writes),
    };
    let mut c = SerialClient::new(Box::new(port), 1, 19200, 11).with_protocol(Protocol::Ascii);
    assert_eq!(c.read_transaction(0, 1).unwrap(), [42]);
    assert_eq!(writes.lock().unwrap()[0], b":010300000001FB\r\n");
}

#[test]
fn ascii_lrc_retry_and_partial_timeout_evidence() {
    let mut c = client(vec![
        b":010302002AD1\r\n".to_vec(),
        b":010302002AD0\r\n".to_vec(),
    ])
    .with_protocol(Protocol::Ascii);
    assert_eq!(c.read_transaction(12, 1).unwrap(), [42]);
    assert_eq!(
        (
            c.stats.lrc_errors,
            c.stats.crc_errors,
            c.stats.recovered,
            c.stats.retries
        ),
        (1, 0, 1, 1)
    );
    let mut c = client(vec![b":0103".to_vec(); 3]).with_protocol(Protocol::Ascii);
    assert!(c
        .read_transaction(12, 1)
        .unwrap_err()
        .to_string()
        .contains("rx=[3A, 30, 31, 30, 33]"));
    assert_eq!((c.stats.timeouts, c.stats.retries), (3, 2));
}

#[test]
fn ascii_rejects_framing_station_function_and_count() {
    for response in [
        b":010302002AD0\n".to_vec(),
        b":01030Z002AD0\r\n".to_vec(),
        ascii_frame(&[2, 3, 2, 0, 42]),
        ascii_frame(&[1, 6, 0, 0, 0, 1]),
        ascii_frame(&[1, 3, 4, 0, 1, 0, 2]),
        ascii_frame(&[1, 0x83, 2]),
        b":0\r\n".to_vec(),
        vec![b':'; 514],
    ] {
        let mut c = client(vec![response]).with_protocol(Protocol::Ascii);
        assert!(c.read_transaction(0, 1).is_err());
        assert_eq!(c.stats.retries, 0);
    }
}

#[test]
fn ascii_write_echo_and_no_retry_on_lrc_or_mismatch() {
    for response in [
        b":0106000C002ABD\r\n".to_vec(),
        ascii_frame(&[1, 6, 0, 12, 0, 41]),
        Vec::new(),
    ] {
        let writes = Arc::default();
        let port = ScriptPort {
            responses: vec![response].into(),
            current: VecDeque::new(),
            writes: Arc::clone(&writes),
        };
        let mut c = SerialClient::new(Box::new(port), 1, 19200, 11).with_protocol(Protocol::Ascii);
        assert!(c.write_single_register(12, 42).is_err());
        assert_eq!(writes.lock().unwrap().len(), 1);
    }
    let mut c = client(vec![b":0106000C002AC3\r\n".to_vec()]).with_protocol(Protocol::Ascii);
    assert!(c.write_single_register(12, 42).is_ok());
}

#[test]
fn ascii_discovery_requires_two_responses() {
    let response = b":010302002AD0\r\n".to_vec();
    assert!(client(vec![response.clone(), response.clone()])
        .with_protocol(Protocol::Ascii)
        .detect_slave(1, 0)
        .unwrap());
    assert!(!client(vec![response])
        .with_protocol(Protocol::Ascii)
        .detect_slave(1, 0)
        .unwrap());
}

#[test]
fn rtu_bad_header_is_rejected_without_waiting_for_tail_or_retrying() {
    for response in [vec![2, 3], vec![1, 6], vec![1, 3, 4]] {
        let mut c = client(vec![response]);
        assert!(c.read_transaction(0, 1).is_err());
        assert_eq!((c.stats.retries, c.stats.timeouts), (0, 0));
    }
}
