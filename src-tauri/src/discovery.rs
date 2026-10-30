use crate::{modbus::Protocol, profile::ParitySetting};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerialMode {
    pub protocol: Protocol,
    pub parity: ParitySetting,
    pub stop_bits: u8,
}

pub fn modes(preferred: SerialMode) -> Vec<SerialMode> {
    let mut result = vec![preferred];
    for protocol in [Protocol::Rtu, Protocol::Ascii] {
        for parity in [ParitySetting::Even, ParitySetting::None, ParitySetting::Odd] {
            let mode = SerialMode {
                protocol,
                parity,
                stop_bits: 1,
            };
            if !result.contains(&mode) {
                result.push(mode);
            }
        }
    }
    result
}

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryStatus {
    pub completed: usize,
    pub total: usize,
    pub slave_id: Option<u8>,
    pub baud_rate: Option<u32>,
    pub serial_mode: Option<SerialMode>,
    pub found: bool,
    pub cancelled: bool,
}

#[derive(Default)]
pub struct DiscoveryControl {
    pub running: AtomicBool,
    pub cancel: AtomicBool,
    pub status: Mutex<DiscoveryStatus>,
}

pub struct DiscoveryLease(pub std::sync::Arc<DiscoveryControl>);
impl Drop for DiscoveryLease {
    fn drop(&mut self) {
        self.0.running.store(false, Ordering::SeqCst);
    }
}

pub const COMMON_BAUD_RATES: [u32; 8] = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];

/// Validate a user-selected baud list against what the current preset allows.
/// The preferred baud is always probed first and the result is de-duplicated.
pub fn validate_selected_rates(
    preferred: u32,
    selected: &[u32],
    allowed: &[u32],
) -> Result<Vec<u32>, String> {
    if selected.is_empty() || selected.len() > 32 {
        return Err("搜索波特率数量须为 1..32".into());
    }
    if !allowed.contains(&preferred) {
        return Err(format!("首选波特率 {preferred} 不在当前允许范围"));
    }
    if !selected.contains(&preferred) {
        return Err("首选波特率必须包含在搜索范围".into());
    }
    let mut rates = vec![preferred];
    for &rate in selected {
        if !allowed.contains(&rate) {
            return Err(format!("波特率 {rate} 不在当前允许范围"));
        }
        if !rates.contains(&rate) {
            rates.push(rate);
        }
    }
    Ok(rates)
}

/// Build the full probe order. The current station is checked across every
/// selected baud and serial mode first, so a single device with an unknown
/// baud/format is found without sweeping the whole station range at one rate.
pub fn plan(
    start: u8,
    end: u8,
    preferred_station: u8,
    preferred_baud: u32,
    rates: &[u32],
    modes: &[SerialMode],
) -> Vec<(u32, u8, SerialMode)> {
    let mut ordered_rates = vec![preferred_baud];
    for &rate in rates {
        if !ordered_rates.contains(&rate) {
            ordered_rates.push(rate);
        }
    }
    let mut stations = Vec::new();
    if (start..=end).contains(&preferred_station) {
        stations.push(preferred_station);
    }
    for station in start..=end {
        if station != preferred_station {
            stations.push(station);
        }
    }
    let Some((&first, rest)) = stations.split_first() else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for &rate in &ordered_rates {
        for &mode in modes {
            result.push((rate, first, mode));
        }
    }
    for &rate in &ordered_rates {
        for &station in rest {
            for &mode in modes {
                result.push((rate, station, mode));
            }
        }
    }
    result
}

pub fn scan(
    control: &DiscoveryControl,
    candidates: &[(u32, u8, SerialMode)],
    mut probe: impl FnMut(u32, u8, SerialMode) -> Result<bool, String>,
) -> Result<DiscoveryStatus, String> {
    let mut status = DiscoveryStatus {
        total: candidates.len(),
        ..Default::default()
    };
    for &(baud, slave, mode) in candidates {
        if control.cancel.load(Ordering::SeqCst) {
            status.cancelled = true;
            break;
        }
        status.serial_mode = Some(mode);
        status.slave_id = Some(slave);
        status.baud_rate = Some(baud);
        *control.status.lock().map_err(|_| "探测状态锁已损坏")? = status.clone();
        let found = probe(baud, slave, mode)?;
        status.completed += 1;
        if control.cancel.load(Ordering::SeqCst) {
            status.cancelled = true;
            break;
        }
        if found {
            status.found = true;
            break;
        }
    }
    *control.status.lock().map_err(|_| "探测状态锁已损坏")? = status.clone();
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scan(
        control: &DiscoveryControl,
        candidates: &[(u32, u8)],
        mut probe: impl FnMut(u32, u8) -> Result<bool, String>,
    ) -> Result<DiscoveryStatus, String> {
        let mode = SerialMode {
            protocol: Protocol::Rtu,
            parity: ParitySetting::Even,
            stop_bits: 1,
        };
        super::scan(
            control,
            &candidates
                .iter()
                .map(|&(b, s)| (b, s, mode))
                .collect::<Vec<_>>(),
            |b, s, _| probe(b, s),
        )
    }
    #[test]
    fn six_modes_include_ascii_and_prefer_current() {
        let preferred = SerialMode {
            protocol: Protocol::Ascii,
            parity: ParitySetting::Odd,
            stop_bits: 1,
        };
        let list = modes(preferred);
        assert_eq!(list.len(), 6);
        assert_eq!(list[0], preferred);
        assert_eq!(
            modes(SerialMode {
                stop_bits: 2,
                ..preferred
            })
            .len(),
            7
        );
    }
    #[test]
    fn selected_rates_are_deduped_and_bounded() {
        assert_eq!(
            validate_selected_rates(19200, &[19200, 9600, 19200], &[9600, 19200]).unwrap(),
            vec![19200, 9600]
        );
        assert!(validate_selected_rates(19200, &[9600], &[9600, 19200]).is_err());
        assert!(validate_selected_rates(19200, &[19200, 12345], &[19200]).is_err());
        assert!(validate_selected_rates(19200, &[], &[19200]).is_err());
    }

    #[test]
    fn plan_checks_the_current_station_across_rates_and_modes_first() {
        let mode = SerialMode {
            protocol: Protocol::Rtu,
            parity: ParitySetting::Even,
            stop_bits: 1,
        };
        let modes = vec![mode];
        let plan = super::plan(1, 3, 2, 19200, &[19200, 9600], &modes);
        assert_eq!(plan.len(), 3 * 2);
        assert_eq!(plan[0], (19200, 2, mode));
        assert_eq!(plan[1], (9600, 2, mode));
        assert_eq!(plan[2], (19200, 1, mode));
        assert_eq!(plan[3], (19200, 3, mode));
        assert_eq!(plan[4], (9600, 1, mode));
        let plan = super::plan(3, 5, 1, 19200, &[19200], &modes);
        assert_eq!(plan[0].1, 3);
    }
    #[test]
    fn found_stops_scan_and_cancel_wins_over_inflight_result() {
        let control = DiscoveryControl::default();
        let result = scan(&control, &[(9600, 1), (19200, 1), (19200, 2)], |baud, _| {
            Ok(baud == 19200)
        })
        .unwrap();
        assert!(result.found);
        assert_eq!(result.completed, 2);
        let result = scan(&control, &[(9600, 1)], |_, _| {
            control.cancel.store(true, Ordering::SeqCst);
            Ok(true)
        })
        .unwrap();
        assert!(result.cancelled);
        assert!(!result.found);
    }
    #[test]
    fn no_match_and_fatal_port_error() {
        let control = DiscoveryControl::default();
        assert!(
            !scan(&control, &[(9600, 1)], |_, _| Ok(false))
                .unwrap()
                .found
        );
        assert!(scan(&control, &[(9600, 1)], |_, _| Err("port busy".into())).is_err());
    }
}
