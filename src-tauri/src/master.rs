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
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlaveDiagnostics {
    pub position: u16,
    pub station: u16,
    pub state: u16,
    pub al_code: u16,
    pub active_ports: u8,
    pub parent_port: u8,
    pub entry_port: u8,
    pub dc_active: bool,
    pub dc_next: u16,
    pub error_counters_valid: bool,
    pub error_counters: Vec<u8>,
}

/// Read-only PRE-OP diagnostics: AL state, topology ports, DC state and the raw
/// ESC error-counter register range 0x0300..0x0311. No OP transition or output.
#[tauri::command]
pub async fn ethercat_diagnostics(
    slave: u16,
    state: State<'_, Master>,
) -> Result<SlaveDiagnostics, String> {
    let master = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let inner = master.inner.lock().map_err(|_| "主站锁损坏")?;
        if !inner.status.connected {
            return Err("EtherCAT 尚未连接".into());
        }
        let device = inner
            .status
            .slaves
            .iter()
            .find(|item| item.position == slave)
            .ok_or("从站不存在")?;
        #[cfg(windows)]
        {
            let diag = native::diagnostics(slave)?;
            Ok(SlaveDiagnostics {
                position: device.position,
                station: device.station,
                state: device.state,
                al_code: diag.al_code,
                active_ports: diag.active_ports,
                parent_port: diag.parent_port,
                entry_port: diag.entry_port,
                dc_active: diag.dc_active != 0,
                dc_next: diag.dc_next,
                error_counters_valid: diag.errors_valid != 0,
                error_counters: diag.error_counters.to_vec(),
            })
        }
        #[cfg(not(windows))]
        {
            let _ = device;
            Err("主站仅支持 Windows".into())
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

fn c_name(bytes: &[u8; 64]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches('\0')
        .trim()
        .to_string()
}

fn coe_limits(size: u8, signed: bool) -> (String, String) {
    if signed {
        match size {
            1 => ("-128".into(), "127".into()),
            2 => ("-32768".into(), "32767".into()),
            4 => ("-2147483648".into(), "2147483647".into()),
            _ => ("-9223372036854775808".into(), "9223372036854775807".into()),
        }
    } else {
        match size {
            1 => ("0".into(), "255".into()),
            2 => ("0".into(), "65535".into()),
            4 => ("0".into(), "4294967295".into()),
            _ => ("0".into(), "18446744073709551615".into()),
        }
    }
}

/// Map one CoE object-dictionary entry to the app's integer object contract.
/// Floats, strings, domains and bit-sized entries are intentionally skipped.
fn coe_object(
    index: u16,
    sub: u8,
    name: &str,
    data_type: u16,
    bit_length: u16,
    access: u16,
) -> Option<Object> {
    let size = match bit_length {
        8 => 1,
        16 => 2,
        32 => 4,
        64 => 8,
        _ => return None,
    };
    let signed = match data_type {
        0x0002 | 0x0003 | 0x0004 | 0x0014 => true,
        0x0005 | 0x0006 | 0x0007 | 0x0019 => false,
        _ => return None,
    };
    let (min, max) = coe_limits(size, signed);
    let fallback = format!("对象 {index:04X}:{sub:02X}");
    Some(Object {
        id: format!("{index:04X}:{sub:02X}"),
        name: if name.is_empty() {
            fallback
        } else {
            name.to_string()
        },
        index,
        sub_index: sub,
        size,
        signed,
        writable: access & 0x02 != 0,
        min,
        max,
        scale: 1.0,
        unit: String::new(),
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EthercatDiscovery {
    pub profile: MasterProfile,
    pub total: usize,
    pub included: usize,
    pub skipped: usize,
    pub failed: usize,
}

/// Read the live CoE object dictionary through SDO Info and build a Profile.
/// This is read-only; it never writes an SDO and never requests OP.
#[tauri::command]
pub async fn ethercat_discover_profile(
    slave: u16,
    state: State<'_, Master>,
) -> Result<EthercatDiscovery, String> {
    let master = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let inner = master.inner.lock().map_err(|_| "主站锁损坏")?;
        if !inner.status.connected {
            return Err("EtherCAT 尚未连接".into());
        }
        let device = inner
            .status
            .slaves
            .iter()
            .find(|item| item.position == slave)
            .ok_or("从站不存在")?;
        if device.mailbox_protocols & 4 == 0 {
            return Err("从站未声明 CoE，无法读取对象字典".into());
        }
        #[cfg(windows)]
        {
            let count = native::od_count(slave)?;
            if count == 0 {
                return Err("从站返回空对象字典".into());
            }
            let mut objects = Vec::new();
            let mut failed = 0usize;
            for item in 0..count {
                let Ok(description) = native::od_item(slave, item as u16) else {
                    failed += 1;
                    continue;
                };
                let Ok(entries) = native::od_entries(slave, item as u16, 256) else {
                    failed += 1;
                    continue;
                };
                for entry in entries {
                    if let Some(object) = coe_object(
                        description.index,
                        entry.sub_index,
                        &c_name(&entry.name),
                        entry.data_type,
                        entry.bit_length,
                        entry.access,
                    ) {
                        objects.push(object);
                    }
                }
            }
            let total = objects.len();
            objects.truncate(512);
            let included = objects.len();
            let skipped = total - included;
            if included == 0 {
                return Err("未发现可用的 8/16/32/64 位整数对象".into());
            }
            let profile = MasterProfile {
                schema_version: "ethercat-1.0".into(),
                name: format!("{}（从设备生成）", device.name),
                vendor: device.vendor,
                product: device.product,
                objects,
            };
            profile.validate()?;
            Ok(EthercatDiscovery {
                profile,
                total,
                included,
                skipped,
                failed,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = device;
            Err("主站仅支持 Windows".into())
        }
    })
    .await
    .map_err(|error| error.to_string())?
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
    fn coe_object_keeps_supported_integers_and_skips_other_types() {
        let gain = coe_object(0x2000, 0, "Gain", 0x0003, 16, 0x03).unwrap();
        assert_eq!(gain.id, "2000:00");
        assert_eq!(gain.name, "Gain");
        assert_eq!(gain.size, 2);
        assert!(gain.signed);
        assert!(gain.writable);
        assert_eq!((gain.min.as_str(), gain.max.as_str()), ("-32768", "32767"));
        let counter = coe_object(0x2001, 1, "", 0x0007, 32, 0x01).unwrap();
        assert_eq!(counter.name, "对象 2001:01");
        assert_eq!(counter.size, 4);
        assert!(!counter.signed);
        assert!(!counter.writable);
        assert_eq!(counter.max, "4294967295");
        let big = coe_object(0x2002, 0, "Big", 0x0014, 64, 0x01).unwrap();
        assert_eq!(big.min, "-9223372036854775808");
        assert!(coe_object(0x2003, 0, "Float", 0x0008, 32, 0x03).is_none());
        assert!(coe_object(0x2004, 0, "Text", 0x0009, 0, 0x01).is_none());
        assert!(coe_object(0x2005, 0, "Bit", 0x0001, 1, 0x01).is_none());
        assert!(coe_object(0x2006, 0, "Odd", 0x0002, 24, 0x01).is_none());
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
