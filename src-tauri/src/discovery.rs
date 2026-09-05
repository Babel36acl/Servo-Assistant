use serde::Serialize;
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

pub fn candidates(
    start: u8,
    end: u8,
    preferred: u8,
    baud: u32,
    allowed: &[u32],
) -> Result<Vec<(u32, u8)>, String> {
    if start == 0 || start > end || end > 247 || !allowed.contains(&baud) {
        return Err("站号范围须为 1..247，首选波特率须属于 Profile".into());
    }
    let mut rates = vec![baud];
    for &rate in allowed {
        if !rates.contains(&rate) {
            rates.push(rate);
        }
    }
    let mut stations = Vec::new();
    if (start..=end).contains(&preferred) {
        stations.push(preferred);
    }
    for station in start..=end {
        if station != preferred {
            stations.push(station);
        }
    }
    Ok(rates
        .into_iter()
        .flat_map(|rate| {
            stations
                .iter()
                .map(move |&station| (rate, station))
                .collect::<Vec<_>>()
        })
        .collect())
}

pub fn scan(
    control: &DiscoveryControl,
    candidates: &[(u32, u8)],
    mut probe: impl FnMut(u32, u8) -> Result<bool, String>,
) -> Result<DiscoveryStatus, String> {
    let mut status = DiscoveryStatus {
        total: candidates.len(),
        ..Default::default()
    };
    for &(baud, slave) in candidates {
        if control.cancel.load(Ordering::SeqCst) {
            status.cancelled = true;
            break;
        }
        status.slave_id = Some(slave);
        status.baud_rate = Some(baud);
        *control.status.lock().map_err(|_| "探测状态锁已损坏")? = status.clone();
        let found = probe(baud, slave)?;
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
    #[test]
    fn ordering_and_limits() {
        assert_eq!(
            candidates(1, 2, 2, 19200, &[9600, 19200, 9600]).unwrap(),
            vec![(19200, 2), (19200, 1), (9600, 2), (9600, 1)]
        );
        assert!(candidates(0, 2, 1, 19200, &[19200]).is_err());
        assert!(candidates(2, 1, 1, 19200, &[19200]).is_err());
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
