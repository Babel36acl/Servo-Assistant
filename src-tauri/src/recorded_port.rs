use crate::recording::Recorder;
use serialport::*;
use std::{
    io::{self, Read, Write},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

pub struct RecordedPort {
    pub port: Box<dyn SerialPort>,
    pub recorder: Recorder,
    pub transaction: Arc<AtomicU64>,
    pub source: String,
}
impl RecordedPort {
    fn log(&self, direction: &str, data: &[u8], detail: &str) {
        self.recorder.emit(
            &self.source,
            "modbus-rtu",
            direction,
            self.transaction.load(Ordering::Relaxed),
            data,
            detail,
        );
    }
}
impl Read for RecordedPort {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        match self.port.read(bytes) {
            Ok(n) => {
                self.log(
                    "rx",
                    &bytes[..n],
                    "I/O chunk; timestamp at application read",
                );
                Ok(n)
            }
            Err(e) => {
                self.log("event", &[], &format!("read: {e}"));
                Err(e)
            }
        }
    }
}
impl Write for RecordedPort {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self.port.write(bytes) {
            Ok(n) => {
                self.log(
                    "tx",
                    &bytes[..n],
                    "accepted by OS; physical transmission unverified",
                );
                Ok(n)
            }
            Err(e) => {
                self.log("event", &[], &format!("write: {e}"));
                Err(e)
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        let result = self.port.flush();
        self.log("event", &[], &format!("flush: {result:?}"));
        result
    }
}
macro_rules! delegate {
    ($name:ident, $result:ty) => {
        fn $name(&self) -> $result {
            self.port.$name()
        }
    };
    ($name:ident, $arg:ident: $ty:ty, $result:ty) => {
        fn $name(&mut self, $arg: $ty) -> $result {
            self.port.$name($arg)
        }
    };
}
impl SerialPort for RecordedPort {
    delegate!(name, Option<String>);
    delegate!(baud_rate, serialport::Result<u32>);
    delegate!(data_bits, serialport::Result<DataBits>);
    delegate!(flow_control, serialport::Result<FlowControl>);
    delegate!(parity, serialport::Result<Parity>);
    delegate!(stop_bits, serialport::Result<StopBits>);
    delegate!(timeout, Duration);
    delegate!(set_baud_rate, value: u32, serialport::Result<()>);
    delegate!(set_data_bits, value: DataBits, serialport::Result<()>);
    delegate!(set_flow_control, value: FlowControl, serialport::Result<()>);
    delegate!(set_parity, value: Parity, serialport::Result<()>);
    delegate!(set_stop_bits, value: StopBits, serialport::Result<()>);
    delegate!(set_timeout, value: Duration, serialport::Result<()>);
    delegate!(write_request_to_send, value: bool, serialport::Result<()>);
    delegate!(write_data_terminal_ready, value: bool, serialport::Result<()>);
    fn read_clear_to_send(&mut self) -> serialport::Result<bool> {
        self.port.read_clear_to_send()
    }
    fn read_data_set_ready(&mut self) -> serialport::Result<bool> {
        self.port.read_data_set_ready()
    }
    fn read_ring_indicator(&mut self) -> serialport::Result<bool> {
        self.port.read_ring_indicator()
    }
    fn read_carrier_detect(&mut self) -> serialport::Result<bool> {
        self.port.read_carrier_detect()
    }
    delegate!(bytes_to_read, serialport::Result<u32>);
    delegate!(bytes_to_write, serialport::Result<u32>);
    fn clear(&self, buffer: ClearBuffer) -> serialport::Result<()> {
        let pending = self.port.bytes_to_read();
        let result = self.port.clear(buffer);
        self.log(
            "event",
            &[],
            &format!(
                "clear {buffer:?}: {result:?}; pending={pending:?}; discarded bytes unavailable"
            ),
        );
        result
    }
    fn try_clone(&self) -> serialport::Result<Box<dyn SerialPort>> {
        Ok(Box::new(Self {
            port: self.port.try_clone()?,
            recorder: self.recorder.clone(),
            transaction: self.transaction.clone(),
            source: self.source.clone(),
        }))
    }
    fn set_break(&self) -> serialport::Result<()> {
        self.port.set_break()
    }
    fn clear_break(&self) -> serialport::Result<()> {
        self.port.clear_break()
    }
}
