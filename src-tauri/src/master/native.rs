//! Windows 原生 FFI：SOEM 主站与 Npcap 抓包的 C 桥，非 Windows 不编译。
use super::*;
use std::{
    ffi::{c_char, c_void},
    sync::OnceLock,
};
type CaptureContext = Mutex<Option<(Recorder, String)>>;
pub fn capture_context() -> &'static CaptureContext {
    static CONTEXT: OnceLock<CaptureContext> = OnceLock::new();
    CONTEXT.get_or_init(Mutex::default)
}
pub extern "C" fn on_frame(direction: i32, data: *const u8, size: u32, stamp: u64) {
    // Native callback is synchronous, data remains valid only until return.
    if data.is_null() || size > 65536 {
        return;
    }
    if let Ok(context) = capture_context().lock() {
        if let Some((recorder, source)) = context.as_ref() {
            let bytes = unsafe { std::slice::from_raw_parts(data, size as usize) };
            recorder.emit_at(
                source,
                "ethernet",
                match direction {
                    1 => "tx",
                    2 => "rx",
                    _ => "event",
                },
                0,
                bytes,
                if direction == 3 {
                    "send failed; transmission unknown"
                } else {
                    "SOEM raw frame"
                },
                if stamp == 0 {
                    crate::recording::timestamp_us()
                } else {
                    stamp
                },
            );
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
struct NativeSlave {
    vendor: u32,
    product: u32,
    revision: u32,
    position: u16,
    station: u16,
    state: u16,
    al_code: u16,
    mailbox_out: u16,
    mailbox_in: u16,
    mailbox_protocols: u16,
    name: [u8; 42],
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeSlaveDiagnostics {
    pub active_ports: u8,
    pub parent_port: u8,
    pub entry_port: u8,
    pub dc_active: u8,
    pub errors_valid: u8,
    pub dc_next: u16,
    pub al_code: u16,
    pub error_counters: [u8; 18],
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeOdItem {
    pub index: u16,
    pub data_type: u16,
    pub object_code: u8,
    pub max_sub: u8,
    pub name: [u8; 64],
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeOdEntry {
    pub data_type: u16,
    pub bit_length: u16,
    pub access: u16,
    pub sub_index: u8,
    pub name: [u8; 64],
}
unsafe extern "C" {
    #[cfg(test)]
    pub fn sa_available() -> i32;
    pub fn sa_adapters(out: *mut c_char, capacity: i32) -> i32;
    pub fn sa_open(
        adapter: *const c_char,
        callback: extern "C" fn(i32, *const u8, u32, u64),
    ) -> i32;
    pub fn sa_close();
    fn sa_slaves(out: *mut NativeSlave, capacity: i32) -> i32;
    fn sa_slave_diagnostics(slave: u16, out: *mut NativeSlaveDiagnostics) -> i32;
    fn sa_od_count(slave: u16) -> i32;
    fn sa_od_item(slave: u16, item: u16, out: *mut NativeOdItem) -> i32;
    fn sa_od_entries(slave: u16, item: u16, out: *mut NativeOdEntry, capacity: i32) -> i32;
    fn sa_sdo_read(slave: u16, index: u16, sub: u8, out: *mut u8, size: *mut i32) -> i32;
    fn sa_sdo_write(slave: u16, index: u16, sub: u8, data: *const u8, size: i32) -> i32;
    fn sa_error(out: *mut c_char, capacity: i32) -> i32;
    pub fn sa_drops() -> i32;
    pub fn sa_capture_open(adapter: *const c_char, error: *mut c_char) -> *mut c_void;
    pub fn sa_capture_next(
        handle: *mut c_void,
        out: *mut u8,
        capacity: u32,
        stamp: *mut u64,
        original: *mut u32,
    ) -> i32;
    pub fn sa_capture_drops(handle: *mut c_void) -> i32;
    pub fn sa_capture_close(handle: *mut c_void);
}
fn error() -> String {
    let mut b = [0u8; 4096];
    let n = unsafe { sa_error(b.as_mut_ptr().cast(), 4096) };
    if n > 0 {
        String::from_utf8_lossy(&b[..n as usize]).into()
    } else {
        "SDO 无有效响应或 Working Counter 无效；写入结果可能未知".into()
    }
}
pub fn slaves() -> Result<Vec<Slave>, String> {
    let mut b = vec![unsafe { std::mem::zeroed::<NativeSlave>() }; 200];
    let n = unsafe { sa_slaves(b.as_mut_ptr(), 200) };
    if n < 0 {
        return Err("从站状态读取失败；当前数据显示已过期".into());
    }
    Ok(b[..n as usize]
        .iter()
        .map(|s| Slave {
            position: s.position,
            station: s.station,
            name: String::from_utf8_lossy(&s.name)
                .trim_end_matches('\0')
                .into(),
            vendor: s.vendor,
            product: s.product,
            revision: s.revision,
            state: s.state,
            al_code: s.al_code,
            mailbox_out: s.mailbox_out,
            mailbox_in: s.mailbox_in,
            mailbox_protocols: s.mailbox_protocols,
        })
        .collect())
}
pub fn diagnostics(slave: u16) -> Result<NativeSlaveDiagnostics, String> {
    let mut value = NativeSlaveDiagnostics {
        active_ports: 0,
        parent_port: 0,
        entry_port: 0,
        dc_active: 0,
        errors_valid: 0,
        dc_next: 0,
        al_code: 0,
        error_counters: [0; 18],
    };
    let result = unsafe { sa_slave_diagnostics(slave, &mut value) };
    if result < 0 {
        return Err("从站诊断读取失败；当前主站状态可能已过期".into());
    }
    Ok(value)
}
pub fn od_count(slave: u16) -> Result<usize, String> {
    let count = unsafe { sa_od_count(slave) };
    if count < 0 {
        return Err("SDO Info 对象字典读取失败；从站可能未声明 CoE".into());
    }
    Ok(count as usize)
}
pub fn od_item(slave: u16, item: u16) -> Result<NativeOdItem, String> {
    let mut value = NativeOdItem {
        index: 0,
        data_type: 0,
        object_code: 0,
        max_sub: 0,
        name: [0; 64],
    };
    let result = unsafe { sa_od_item(slave, item, &mut value) };
    if result < 0 {
        return Err("对象描述读取失败".into());
    }
    Ok(value)
}
pub fn od_entries(
    slave: u16,
    item: u16,
    capacity: usize,
) -> Result<Vec<NativeOdEntry>, String> {
    let mut values = vec![
        NativeOdEntry {
            data_type: 0,
            bit_length: 0,
            access: 0,
            sub_index: 0,
            name: [0; 64],
        };
        capacity
    ];
    let count = unsafe { sa_od_entries(slave, item, values.as_mut_ptr(), capacity as i32) };
    if count < 0 {
        return Err("对象条目读取失败".into());
    }
    values.truncate(count as usize);
    Ok(values)
}
pub fn read(slave: u16, index: u16, sub: u8) -> Result<Vec<u8>, String> {
    let _ = error();
    let mut b = vec![0u8; 4096];
    let mut size = 4096;
    let n = unsafe { sa_sdo_read(slave, index, sub, b.as_mut_ptr(), &mut size) };
    if n <= 0 {
        return Err(error());
    }
    if !(0..=4096).contains(&size) {
        return Err("SDO 返回长度越界".into());
    }
    b.truncate(size as usize);
    Ok(b)
}
pub fn write(slave: u16, index: u16, sub: u8, data: &[u8]) -> Result<(), String> {
    let _ = error();
    let n = unsafe { sa_sdo_write(slave, index, sub, data.as_ptr(), data.len() as i32) };
    if n <= 0 {
        Err(error())
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abi_layout() {
        assert_eq!(std::mem::size_of::<NativeSlave>(), 68);
        assert_eq!(std::mem::size_of::<NativeSlaveDiagnostics>(), 28);
        assert_eq!(std::mem::size_of::<NativeOdItem>(), 70);
        assert_eq!(std::mem::size_of::<NativeOdEntry>(), 72);
    }
    #[test]
    fn missing_driver_is_reported_without_loading_failure() {
        let available = unsafe { sa_available() };
        assert!(available == 0 || available == 1);
    }
}
