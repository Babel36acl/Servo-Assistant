//! Configuration/monitoring EtherCAT master. SOEM owns mailbox transactions;
//! application owns authorization, typed object contract and compare/readback.
use crate::{audit::AuditStore, recording::Recorder};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::State;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Object {
    pub id: String,
    pub name: String,
    pub index: u16,
    pub sub_index: u8,
    pub size: u8,
    #[serde(default)]
    pub signed: bool,
    #[serde(default)]
    pub writable: bool,
    pub min: String,
    pub max: String,
    #[serde(default = "scale_one")]
    pub scale: f64,
    #[serde(default)]
    pub unit: String,
}
fn scale_one() -> f64 {
    1.0
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MasterProfile {
    pub schema_version: String,
    pub name: String,
    pub vendor: u32,
    pub product: u32,
    pub objects: Vec<Object>,
}
impl MasterProfile {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != "ethercat-1.0"
            || self.objects.is_empty()
            || self.objects.len() > 512
        {
            return Err("EtherCAT Profile 版本或对象数量无效".into());
        }
        let mut ids = std::collections::HashSet::new();
        let mut addresses = std::collections::HashSet::new();
        for o in &self.objects {
            if o.id.is_empty()
                || !ids.insert(&o.id)
                || !addresses.insert((o.index, o.sub_index))
                || !matches!(o.size, 1 | 2 | 4 | 8)
                || !o.scale.is_finite()
                || o.scale == 0.0
            {
                return Err("对象 ID、地址、位宽或缩放无效/重复".into());
            }
            let min = o
                .min
                .parse::<i128>()
                .map_err(|_| "对象 min 不是十进制整数")?;
            let max = o
                .max
                .parse::<i128>()
                .map_err(|_| "对象 max 不是十进制整数")?;
            if min > max {
                return Err("对象 min 大于 max".into());
            }
            o.encode(&o.min)?;
            o.encode(&o.max)?;
        }
        Ok(())
    }
}
impl Object {
    fn encode(&self, value: &str) -> Result<Vec<u8>, String> {
        if !matches!(self.size, 1 | 2 | 4 | 8) {
            return Err("对象位宽无效".into());
        }
        let n = value.parse::<i128>().map_err(|_| "请输入原始十进制整数")?;
        let bits = self.size * 8;
        let (low, high) = if self.signed {
            (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
        } else {
            (0, (1i128 << bits) - 1)
        };
        let min = self.min.parse::<i128>().map_err(|_| "min 无效")?;
        let max = self.max.parse::<i128>().map_err(|_| "max 无效")?;
        if n < low || n > high || n < min || n > max {
            return Err("写入值超出类型或 Profile 范围".into());
        }
        Ok(n.to_le_bytes()[..self.size as usize].to_vec())
    }
    fn value(&self, data: &[u8]) -> Result<String, String> {
        if data.len() != self.size as usize {
            return Err(format!(
                "对象 {} 返回 {} 字节，配置要求 {}",
                self.id,
                data.len(),
                self.size
            ));
        }
        let mut bytes = [0u8; 16];
        bytes[..data.len()].copy_from_slice(data);
        if self.signed && data.last().is_some_and(|b| b & 0x80 != 0) {
            bytes[data.len()..].fill(255);
        }
        Ok(i128::from_le_bytes(bytes).to_string())
    }
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Slave {
    pub position: u16,
    pub station: u16,
    pub name: String,
    pub vendor: u32,
    pub product: u32,
    pub revision: u32,
    pub state: u16,
    pub al_code: u16,
    pub mailbox_out: u16,
    pub mailbox_in: u16,
    pub mailbox_protocols: u16,
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MasterStatus {
    pub connected: bool,
    pub adapter: String,
    pub slaves: Vec<Slave>,
    pub driver_dropped: Option<u32>,
}
struct Inner {
    status: MasterStatus,
    profile: Option<MasterProfile>,
}
#[derive(Clone)]
pub struct Master {
    inner: Arc<Mutex<Inner>>,
    recorder: Recorder,
    audit: AuditStore,
}
impl Master {
    pub fn new(recorder: Recorder, audit: AuditStore) -> Self {
        recorder.set_context("ethercatProfile", serde_json::Value::Null);
        recorder.set_context(
            "ethercatConnection",
            serde_json::json!(MasterStatus::default()),
        );
        Self {
            inner: Arc::new(Mutex::new(Inner {
                status: MasterStatus::default(),
                profile: None,
            })),
            recorder,
            audit,
        }
    }
    pub fn close(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            #[cfg(windows)]
            if inner.status.connected {
                unsafe { native::sa_close() };
            }
            inner.status = MasterStatus::default();
            self.recorder
                .set_context("ethercatConnection", serde_json::json!(inner.status));
        }
    }
    fn log(&self, status: &str, detail: &str) {
        self.recorder
            .emit("ethercat-master", "operation", "event", 0, &[], detail);
        if let Err(e) = self.audit.append("ethercat.sdo", status, detail) {
            eprintln!("EtherCAT audit: {e}");
        }
    }
}
#[derive(Serialize)]
pub struct Adapter {
    pub name: String,
    pub description: String,
}
#[tauri::command]
pub async fn ethercat_adapters() -> Result<Vec<Adapter>, String> {
    tauri::async_runtime::spawn_blocking(adapters)
        .await
        .map_err(|e| e.to_string())?
}
pub fn adapters() -> Result<Vec<Adapter>, String> {
    #[cfg(windows)]
    {
        let mut out = vec![0u8; 65536];
        let n = unsafe { native::sa_adapters(out.as_mut_ptr().cast(), out.len() as i32) };
        if n < 0 {
            return Err("无法枚举网卡：请安装 Npcap 后重启应用，并检查抓包权限".into());
        }
        Ok(String::from_utf8_lossy(&out[..n as usize])
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .map(|(name, description)| Adapter {
                name: name.into(),
                description: description.into(),
            })
            .collect())
    }
    #[cfg(not(windows))]
    {
        Err("当前主站后端仅支持 Windows/Npcap".into())
    }
}
#[tauri::command]
pub async fn ethercat_connect(
    adapter: String,
    confirmation: String,
    state: State<'_, Master>,
) -> Result<MasterStatus, String> {
    if confirmation != "接管 EtherCAT 总线" {
        return Err("确认独占网卡及总线后，输入：接管 EtherCAT 总线".into());
    }
    let master = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let inner = master.inner.lock().map_err(|_| "主站锁损坏")?;
        if inner.status.connected {
            return Err("主站已连接".into());
        }
        if !adapters()?.iter().any(|a| a.name == adapter) {
            return Err("网卡不存在，请刷新列表".into());
        }
        #[cfg(windows)]
        {
            let mut inner = inner;
            let name = std::ffi::CString::new(adapter.clone()).map_err(|_| "网卡名称无效")?;
            *native::capture_context().lock().map_err(|_| "捕获锁损坏")? =
                Some((master.recorder.clone(), adapter.clone()));
            let n = unsafe { native::sa_open(name.as_ptr(), native::on_frame) };
            if n <= 0 {
                return Err(format!(
                    "EtherCAT 初始化失败（{n}）：检查 Npcap 权限、独占总线和从站接线"
                ));
            }
            let slaves = match native::slaves() {
                Ok(s) => s,
                Err(e) => {
                    unsafe { native::sa_close() };
                    return Err(e);
                }
            };
            inner.status = MasterStatus {
                connected: true,
                adapter,
                slaves,
                driver_dropped: None,
            };
            master
                .recorder
                .set_context("ethercatConnection", serde_json::json!(inner.status));
            master.log(
                "success",
                "master opened; requested PRE-OP; no PDO outputs or OP transition",
            );
            Ok(inner.status.clone())
        }
        #[cfg(not(windows))]
        {
            Err("主站仅支持 Windows".into())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn ethercat_disconnect(state: State<'_, Master>) -> Result<(), String> {
    let master = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || master.close())
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn ethercat_status(
    refresh: bool,
    state: State<'_, Master>,
) -> Result<MasterStatus, String> {
    let master = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let inner = master.inner.lock().map_err(|_| "主站锁损坏")?;
        #[cfg(windows)]
        let mut inner = inner;
        #[cfg(not(windows))]
        let _ = refresh;
        #[cfg(windows)]
        if refresh && inner.status.connected {
            inner.status.slaves = native::slaves()?;
            let drops = unsafe { native::sa_drops() };
            inner.status.driver_dropped = u32::try_from(drops).ok();
        }
        Ok(inner.status.clone())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn ethercat_profile(
    profile: MasterProfile,
    state: State<'_, Master>,
) -> Result<MasterProfile, String> {
    profile.validate()?;
    state.inner.lock().map_err(|_| "主站锁损坏")?.profile = Some(profile.clone());
    state
        .recorder
        .set_context("ethercatProfile", serde_json::json!(profile));
    Ok(profile)
}
fn selected(inner: &Inner, slave: u16, id: &str) -> Result<Object, String> {
    if !inner.status.connected {
        return Err("EtherCAT 尚未连接".into());
    }
    let device = inner
        .status
        .slaves
        .iter()
        .find(|s| s.position == slave)
        .ok_or("从站不存在")?;
    let profile = inner.profile.as_ref().ok_or("请导入 EtherCAT Profile")?;
    if device.vendor != profile.vendor || device.product != profile.product {
        return Err("从站 Vendor/Product 与 Profile 不匹配".into());
    }
    if device.mailbox_protocols & 4 == 0 {
        return Err("从站未声明 CoE 支持".into());
    }
    profile
        .objects
        .iter()
        .find(|o| o.id == id)
        .cloned()
        .ok_or("对象不存在".into())
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectValue {
    pub id: String,
    pub raw: String,
    pub value: f64,
    pub timestamp_us: u64,
}
fn read_object(slave: u16, object: &Object) -> Result<ObjectValue, String> {
    #[cfg(windows)]
    {
        let data = native::read(slave, object.index, object.sub_index)?;
        let raw = object.value(&data)?;
        Ok(ObjectValue {
            id: object.id.clone(),
            value: raw.parse::<f64>().map_err(|_| "数值转换失败")? * object.scale,
            raw,
            timestamp_us: crate::recording::timestamp_us(),
        })
    }
    #[cfg(not(windows))]
    {
        let _ = (slave, object);
        Err("主站仅支持 Windows".into())
    }
}
#[tauri::command]
pub async fn ethercat_read(
    slave: u16,
    id: String,
    state: State<'_, Master>,
) -> Result<ObjectValue, String> {
    let master = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let inner = master.inner.lock().map_err(|_| "主站锁损坏")?;
        let object = selected(&inner, slave, &id)?;
        read_object(slave, &object)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn ethercat_write(
    slave: u16,
    id: String,
    value: String,
    expected: String,
    confirmation: String,
    state: State<'_, Master>,
) -> Result<ObjectValue, String> {
    let master = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let inner = master.inner.lock().map_err(|_| "主站锁损坏")?;
        let object = selected(&inner, slave, &id)?;
        let result = verified_write(
            &object,
            &value,
            &expected,
            &confirmation,
            || read_object(slave, &object),
            |bytes| {
                // Durable authorization evidence is required BEFORE a physical write.
                master
                    .audit
                    .append(
                        "ethercat.sdo.write",
                        "attempt",
                        &format!("slave={slave} object={id} previous={expected} requested={value}"),
                    )
                    .map_err(|e| e.to_string())?;
                #[cfg(windows)]
                {
                    native::write(slave, object.index, object.sub_index, bytes)
                }
                #[cfg(not(windows))]
                {
                    let _ = bytes;
                    Err("主站仅支持 Windows".into())
                }
            },
        );
        master.log(
            if result.is_ok() { "success" } else { "failure" },
            &format!(
                "slave={slave} object={id} requested={value} readback={:?}",
                result.as_ref().map(|v| &v.raw).map_err(|e| e.as_str())
            ),
        );
        result
    })
    .await
    .map_err(|e| e.to_string())?
}

fn verified_write(
    object: &Object,
    value: &str,
    expected: &str,
    confirmation: &str,
    mut read: impl FnMut() -> Result<ObjectValue, String>,
    write: impl FnOnce(&[u8]) -> Result<(), String>,
) -> Result<ObjectValue, String> {
    if !object.writable || confirmation != format!("写入 {}", object.id) {
        return Err("对象只读或确认短语不匹配".into());
    }
    let bytes = object.encode(value)?;
    if read()?.raw != expected {
        return Err("写前值已变化，请重新读取后确认".into());
    }
    write(&bytes).map_err(|e| format!("写入结果可能未知：{e}；不会自动重写"))?;
    let readback = read().map_err(|e| format!("写入已发送，但回读失败：{e}；不会自动重写"))?;
    if readback.raw != object.value(&bytes)? {
        return Err("写后回读不一致，禁止自动重写".into());
    }
    Ok(readback)
}

#[cfg(windows)]
pub mod native {
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
        }
        #[test]
        fn missing_driver_is_reported_without_loading_failure() {
            let available = unsafe { sa_available() };
            assert!(available == 0 || available == 1);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn object() -> Object {
        Object {
            id: "gain".into(),
            name: "Gain".into(),
            index: 0x2000,
            sub_index: 0,
            size: 2,
            signed: true,
            writable: true,
            min: "-32768".into(),
            max: "32767".into(),
            scale: 0.1,
            unit: "".into(),
        }
    }
    #[test]
    fn signed_ranges_and_precision() {
        let o = object();
        assert_eq!(o.encode("-2").unwrap(), [254, 255]);
        assert_eq!(o.value(&[254, 255]).unwrap(), "-2");
        assert!(o.encode("32768").is_err());
        assert!(o.encode("1.5").is_err());
        assert!(o.value(&[1]).is_err());
    }
    #[test]
    fn duplicate_contract_rejected() {
        let o = object();
        let p = MasterProfile {
            schema_version: "ethercat-1.0".into(),
            name: "test".into(),
            vendor: 1,
            product: 2,
            objects: vec![o.clone(), o],
        };
        assert!(p.validate().is_err());
    }
    fn reading(raw: &str) -> ObjectValue {
        ObjectValue {
            id: "gain".into(),
            raw: raw.into(),
            value: 0.0,
            timestamp_us: 0,
        }
    }
    #[test]
    fn stale_and_unauthorized_never_write() {
        let o = object();
        assert!(verified_write(
            &o,
            "3",
            "1",
            "写入 gain",
            || Ok(reading("2")),
            |_| panic!("stale write")
        )
        .is_err());
        assert!(verified_write(
            &o,
            "3",
            "1",
            "wrong",
            || panic!("unauthorized read"),
            |_| panic!("unauthorized write")
        )
        .is_err());
    }
    #[test]
    fn one_write_and_readback_mismatch_is_not_retried() {
        let o = object();
        let writes = std::cell::Cell::new(0);
        let reads = std::cell::Cell::new(0);
        let result = verified_write(
            &o,
            "3",
            "1",
            "写入 gain",
            || {
                reads.set(reads.get() + 1);
                Ok(reading(if reads.get() == 1 { "1" } else { "2" }))
            },
            |bytes| {
                assert_eq!(bytes, [3, 0]);
                writes.set(writes.get() + 1);
                Ok(())
            },
        );
        assert!(result.unwrap_err().contains("回读不一致"));
        assert_eq!(writes.get(), 1);
        assert_eq!(reads.get(), 2);
    }
    #[test]
    fn transport_failure_stops_without_readback_or_retry() {
        let o = object();
        let reads = std::cell::Cell::new(0);
        let result = verified_write(
            &o,
            "3",
            "1",
            "写入 gain",
            || {
                reads.set(reads.get() + 1);
                Ok(reading("1"))
            },
            |_| Err("timeout".into()),
        );
        assert!(result.unwrap_err().contains("未知"));
        assert_eq!(reads.get(), 1);
    }
}
