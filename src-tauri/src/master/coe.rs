//! CoE 对象字典辅助：把 SDO Info 条目收敛成应用的整数对象契约。

use super::Object;

/// 仅在 Windows 原生路径使用（SDO Info 对象描述的名字字段）。
#[cfg(windows)]
pub(super) fn c_name(bytes: &[u8; 64]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches('\0')
        .trim()
        .to_string()
}

/// 由 `coe_object` 调用：Windows 原生枚举与单元测试都要用到。
pub(super) fn coe_limits(size: u8, signed: bool) -> (String, String) {
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
pub(super) fn coe_object(
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
