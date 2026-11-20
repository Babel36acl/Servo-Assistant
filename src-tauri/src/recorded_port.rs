use crate::modbus::Transport;
use crate::profile::ParitySetting;
use crate::recording::Recorder;
use std::io;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;

/// 包装任意 Transport，把每个方向、每个事件的字节流写进录制器。
pub struct RecordedPort {
    pub port: Box<dyn Transport>,
    pub recorder: Recorder,
    pub transaction: Arc<AtomicU64>,
    pub source: String,
    pub protocol: &'static str,
}
impl RecordedPort {
    fn log(&self, direction: &str, data: &[u8], detail: &str) {
        self.recorder.emit(
            &self.source,
            self.protocol,
            direction,
            self.transaction.load(Ordering::Relaxed),
            data,
            detail,
        );
    }
}
impl Transport for RecordedPort {
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
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        let result = self.port.write_all(bytes);
        match &result {
            Ok(()) => self.log(
                "tx",
                bytes,
                "accepted by OS; physical transmission unverified",
            ),
            Err(e) => self.log("event", &[], &format!("write: {e}")),
        }
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        let result = self.port.flush();
        self.log("event", &[], &format!("flush: {result:?}"));
        result
    }
    fn clear_input(&mut self) -> io::Result<()> {
        let result = self.port.clear_input();
        self.log(
            "event",
            &[],
            &format!("clear input: {result:?}; discarded bytes unavailable"),
        );
        result
    }
    fn set_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        let result = self.port.set_timeout(timeout);
        self.log("event", &[], &format!("set_timeout: {result:?}"));
        result
    }
    fn timeout(&self) -> Duration {
        self.port.timeout()
    }
    fn name(&self) -> String {
        self.port.name()
    }
    fn reconfigure(
        &mut self,
        baud: u32,
        parity: ParitySetting,
        stop_bits: u8,
    ) -> Result<(), String> {
        self.port.reconfigure(baud, parity, stop_bits)
    }
}
