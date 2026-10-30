//! Bounded, read-only Modbus holding-register scanning.
//!
//! The scan sends FC03 reads only. It never writes, never guesses device
//! semantics, and keeps the address span bounded so a scan cannot monopolise
//! the bus for an unbounded time.
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Largest address span accepted by one scan.
pub const MAX_ADDRESSES: u32 = 4096;
/// Largest block accepted by one FC03 transaction.
pub const MAX_BLOCK: u16 = 100;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterValue {
    pub address: u16,
    pub raw: u16,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStatus {
    pub completed: usize,
    pub total: usize,
    pub current_address: Option<u16>,
    pub values: Vec<RegisterValue>,
    pub failed_blocks: usize,
    pub cancelled: bool,
    pub elapsed_ms: u64,
}

#[derive(Default)]
pub struct ScanControl {
    pub running: AtomicBool,
    pub cancel: AtomicBool,
    pub status: Mutex<ScanStatus>,
}

pub struct ScanLease(pub Arc<ScanControl>);
impl Drop for ScanLease {
    fn drop(&mut self) {
        self.0.running.store(false, Ordering::SeqCst);
    }
}

/// Split an inclusive address range into bounded FC03 blocks.
pub fn blocks(start: u16, end: u16, block: u16) -> Result<Vec<(u16, u16)>, String> {
    if start > end {
        return Err("起始地址不能大于结束地址".into());
    }
    if !(1..=MAX_BLOCK).contains(&block) {
        return Err(format!("每块寄存器数须为 1..{MAX_BLOCK}"));
    }
    let span = end as u32 - start as u32 + 1;
    if span > MAX_ADDRESSES {
        return Err(format!("单次扫描地址数不能超过 {MAX_ADDRESSES}"));
    }
    let mut result = Vec::new();
    let mut address = start as u32;
    while address <= end as u32 {
        let count = (end as u32 - address + 1).min(block as u32) as u16;
        result.push((address as u16, count));
        address += count as u32;
    }
    Ok(result)
}

/// Run a read-only scan. A failed block is counted and skipped; the caller keeps
/// the decision about whether a transport failure should stop the whole scan.
pub fn scan(
    control: &ScanControl,
    start: u16,
    end: u16,
    block: u16,
    include_zero: bool,
    mut read: impl FnMut(u16, u16) -> Result<Vec<u16>, String>,
) -> Result<ScanStatus, String> {
    let plan = blocks(start, end, block)?;
    let started = Instant::now();
    let mut status = ScanStatus {
        total: plan.len(),
        ..Default::default()
    };
    for (address, count) in plan {
        if control.cancel.load(Ordering::SeqCst) {
            status.cancelled = true;
            break;
        }
        status.current_address = Some(address);
        *control.status.lock().map_err(|_| "扫描状态锁已损坏")? = status.clone();
        match read(address, count) {
            Ok(values) => {
                for (offset, raw) in values.into_iter().enumerate() {
                    if include_zero || raw != 0 {
                        status.values.push(RegisterValue {
                            address: address + offset as u16,
                            raw,
                        });
                    }
                }
            }
            Err(_) => status.failed_blocks += 1,
        }
        status.completed += 1;
    }
    if control.cancel.load(Ordering::SeqCst) {
        status.cancelled = true;
    }
    status.elapsed_ms = started.elapsed().as_millis() as u64;
    status.current_address = None;
    *control.status.lock().map_err(|_| "扫描状态锁已损坏")? = status.clone();
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_cover_range_and_reject_invalid_input() {
        assert_eq!(blocks(0, 0, 16).unwrap(), vec![(0, 1)]);
        assert_eq!(
            blocks(10, 45, 16).unwrap(),
            vec![(10, 16), (26, 16), (42, 4)]
        );
        assert!(blocks(5, 4, 16).is_err());
        assert!(blocks(0, 10, 0).is_err());
        assert!(blocks(0, 10, 101).is_err());
        assert!(blocks(0, MAX_ADDRESSES as u16, 16).is_err());
    }

    #[test]
    fn scan_collects_non_zero_values_and_counts_failures() {
        let control = ScanControl::default();
        let status = scan(&control, 0, 19, 10, false, |address, count| {
            if address == 0 {
                return Err("timeout".into());
            }
            Ok(vec![0; count as usize])
        })
        .unwrap();
        assert_eq!(status.total, 2);
        assert_eq!(status.completed, 2);
        assert_eq!(status.failed_blocks, 1);
        assert!(status.values.is_empty());

        let control = ScanControl::default();
        let status = scan(&control, 0, 9, 5, false, |address, count| {
            let mut values = vec![0; count as usize];
            if address == 0 {
                values[0] = 7;
            }
            if address == 5 {
                values[1] = 5;
            }
            Ok(values)
        })
        .unwrap();
        assert_eq!(
            status.values,
            vec![
                RegisterValue { address: 0, raw: 7 },
                RegisterValue { address: 6, raw: 5 }
            ]
        );
    }

    #[test]
    fn scan_can_include_zero_values() {
        let control = ScanControl::default();
        let status = scan(&control, 3, 5, 3, true, |_, _| Ok(vec![0, 1, 0])).unwrap();
        assert_eq!(status.values.len(), 3);
        assert_eq!(status.values[1].address, 4);
        assert_eq!(status.values[1].raw, 1);
    }

    #[test]
    fn scan_stops_on_cancel() {
        let control = ScanControl::default();
        control.cancel.store(true, Ordering::SeqCst);
        let status = scan(&control, 0, 100, 10, false, |_, _| Ok(vec![1])).unwrap();
        assert!(status.cancelled);
        assert_eq!(status.completed, 0);
        assert!(status.values.is_empty());
    }

    #[test]
    fn scan_cancel_after_an_inflight_block_marks_cancelled() {
        let control = ScanControl::default();
        let status = scan(&control, 0, 20, 10, false, |_, _| {
            control.cancel.store(true, Ordering::SeqCst);
            Ok(vec![1])
        })
        .unwrap();
        assert!(status.cancelled);
        assert_eq!(status.completed, 1);
        assert_eq!(status.values.len(), 1);
    }
}
