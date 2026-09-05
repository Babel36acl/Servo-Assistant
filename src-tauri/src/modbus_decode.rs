//! Offline decoding only: no transport calls and no inferred bytes from event text.
use crate::{modbus, recording::Record};
use serde::Serialize;

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    pub hex: String,
    pub checksum: String,
    pub slave: Option<u8>,
    pub function: Option<u8>,
    pub address: Option<u16>,
    pub count: Option<u16>,
    pub values: Vec<u16>,
    pub exception: Option<String>,
    pub error: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub source: String,
    pub protocol: String,
    pub transaction: u64,
    pub outcome: String,
    pub duration_us: Option<u64>,
    pub request: Frame,
    pub response: Frame,
    pub events: Vec<String>,
    pub warning: Option<String>,
}

fn payload(bytes: &[u8], protocol: &str) -> Result<Vec<u8>, String> {
    if bytes.is_empty() {
        return Err("未记录到字节".into());
    }
    if protocol == "modbus-ascii" {
        return modbus::decode_ascii(bytes).map_err(|e| e.to_string());
    }
    if bytes.len() < 4 || bytes.len() > 256 {
        return Err("RTU 帧不完整或超过 256 字节".into());
    }
    let end = bytes.len() - 2;
    if modbus::modbus_crc(&bytes[..end]) != u16::from_le_bytes([bytes[end], bytes[end + 1]]) {
        return Err("CRC 校验失败；可能是损坏或缺失片段".into());
    }
    Ok(bytes[..end].to_vec())
}

fn frame(bytes: &[u8], protocol: &str, request: bool) -> Frame {
    let mut f = Frame {
        hex: bytes
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" "),
        checksum: "未通过".into(),
        ..Default::default()
    };
    let p = match payload(bytes, protocol) {
        Ok(p) => p,
        Err(e) => {
            f.error = Some(e);
            return f;
        }
    };
    f.checksum = if protocol == "modbus-ascii" {
        "LRC 通过"
    } else {
        "CRC 通过"
    }
    .into();
    if p.len() < 2 {
        f.error = Some("缺少站号或功能码".into());
        return f;
    }
    f.slave = Some(p[0]);
    f.function = Some(p[1]);
    if !request && p[1] & 0x80 != 0 {
        if p.len() != 3 {
            f.error = Some("异常响应长度不正确".into());
        } else {
            let label = match p[2] {
                1 => "非法功能",
                2 => "非法数据地址",
                3 => "非法数据值",
                4 => "设备故障",
                5 => "确认处理中",
                6 => "设备忙",
                8 => "存储奇偶校验错误",
                10 => "网关路径不可用",
                11 => "网关目标无响应",
                _ => "未知异常码",
            };
            f.exception = Some(format!("0x{:02X} · {label}", p[2]));
        }
        return f;
    }
    match (p[1], request) {
        (3, true) | (6, _) if p.len() == 6 => {
            f.address = Some(u16::from_be_bytes([p[2], p[3]]));
            let value = u16::from_be_bytes([p[4], p[5]]);
            if p[1] == 3 {
                f.count = Some(value);
                if value == 0 || value > 125 || f.address.unwrap().checked_add(value - 1).is_none()
                {
                    f.error = Some("FC03 寄存器范围无效".into());
                }
            } else {
                f.count = Some(1);
                f.values.push(value);
            }
        }
        (3, false)
            if p.len() >= 3
                && p[2] > 0
                && p[2] <= 250
                && p[2] % 2 == 0
                && p.len() == 3 + p[2] as usize =>
        {
            f.count = Some(p[2] as u16 / 2);
            f.values = p[3..]
                .chunks_exact(2)
                .map(|b| u16::from_be_bytes([b[0], b[1]]))
                .collect();
        }
        (3 | 6, _) => f.error = Some("FC03 / FC06 帧长度或字节数无效".into()),
        _ => f.error = Some("当前仅解析 FC03、FC06 及其异常响应；其他功能保留原始字节".into()),
    }
    f
}

pub fn decode(records: &[Record], warning: Option<String>) -> Result<Transaction, String> {
    let first = records.first().ok_or("未找到事务记录")?;
    if first.transaction == 0 || !matches!(first.protocol.as_str(), "modbus-rtu" | "modbus-ascii") {
        return Err("不是可关联的 Modbus 事务".into());
    }
    if records.iter().any(|r| {
        r.transaction != first.transaction
            || r.source != first.source
            || r.protocol != first.protocol
    }) {
        return Err("不能合并不同来源、协议或事务".into());
    }
    let mut tx = Vec::new();
    let mut rx = Vec::new();
    let mut events = Vec::new();
    let mut started = None;
    let mut ended = None;
    let mut terminal = false;
    let mut failed = false;
    for r in records {
        match r.direction.as_str() {
            "tx" => {
                if !r.bytes.is_empty() {
                    started.get_or_insert(r.elapsed_us);
                }
                tx.extend_from_slice(&r.bytes);
            }
            "rx" => {
                rx.extend_from_slice(&r.bytes);
                ended = Some(r.elapsed_us);
            }
            "event" => {
                events.push(r.detail.clone());
                if r.detail.starts_with("FC") && r.detail.contains(" result=") {
                    terminal = true;
                    failed |= r.detail.contains("result=Err(");
                    ended = Some(r.elapsed_us);
                }
            }
            _ => {}
        }
        if tx.len() > 513 || rx.len() > 513 {
            return Err("事务字节超过单帧上限；可能存在多帧或重复事务编号".into());
        }
    }
    let request = frame(&tx, &first.protocol, true);
    let mut response = frame(&rx, &first.protocol, false);
    let mut outcome = if request.error.is_some() || response.error.is_some() {
        "帧不完整或校验/格式失败"
    } else if request.slave != response.slave
        || response.function.map(|f| f & 0x7f) != request.function
    {
        "响应站号或功能码不匹配"
    } else if response.exception.is_some() {
        "设备异常响应"
    } else if request.function == Some(3) && request.count != response.count {
        "响应寄存器数量不匹配"
    } else if request.function == Some(6)
        && (request.address != response.address || request.values != response.values)
    {
        "写入回显不匹配"
    } else {
        "请求与响应匹配"
    }
    .to_string();
    if outcome == "请求与响应匹配" && request.function == Some(3) {
        response.address = request.address;
    }
    if failed {
        outcome = format!("事务失败 · {outcome}");
    }
    if !terminal {
        outcome = format!("未记录到事务结束 · {outcome}");
    }
    Ok(Transaction {
        source: first.source.clone(),
        protocol: first.protocol.clone(),
        transaction: first.transaction,
        outcome,
        duration_us: started.and_then(|s| ended.and_then(|e| e.checked_sub(s))),
        request,
        response,
        events,
        warning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rtu(p: &[u8]) -> Vec<u8> {
        let mut b = p.to_vec();
        b.extend_from_slice(&modbus::modbus_crc(p).to_le_bytes());
        b
    }
    fn record(direction: &str, bytes: Vec<u8>, elapsed_us: u64) -> Record {
        Record {
            sequence: elapsed_us,
            timestamp_us: elapsed_us,
            elapsed_us,
            source: "COM-test/1".into(),
            protocol: "modbus-rtu".into(),
            transaction: 1,
            direction: direction.into(),
            bytes,
            detail: if direction == "event" {
                "FC03 request=[] result=Ok(\"success\")".into()
            } else {
                String::new()
            },
        }
    }
    #[test]
    fn fragmented_read_and_exception_and_bad_crc() {
        let tx = rtu(&[1, 3, 0, 16, 0, 2]);
        let rx = rtu(&[1, 3, 4, 0, 42, 255, 255]);
        let mut rows = vec![
            record("tx", tx, 10),
            record("rx", rx[..2].to_vec(), 20),
            record("rx", rx[2..].to_vec(), 30),
            record("event", vec![], 40),
        ];
        let t = decode(&rows, None).unwrap();
        assert_eq!(t.response.values, [42, 65535]);
        assert_eq!(t.response.address, Some(16));
        assert_eq!(t.duration_us, Some(30));
        assert_eq!(t.outcome, "请求与响应匹配");
        rows[1].bytes = rtu(&[1, 0x83, 2]);
        rows[2].bytes.clear();
        assert!(decode(&rows, None)
            .unwrap()
            .response
            .exception
            .unwrap()
            .contains("非法数据地址"));
        rows[1].bytes[0] ^= 1;
        assert!(decode(&rows, None)
            .unwrap()
            .response
            .error
            .unwrap()
            .contains("CRC"));
    }
    #[test]
    fn ascii_lrc_write_echo_and_missing_response() {
        let mut rows = vec![
            record("tx", b":01060010002ABF\r\n".to_vec(), 10),
            record("rx", b":01060010002ABF\r\n".to_vec(), 20),
            record("event", vec![], 30),
        ];
        for r in &mut rows {
            r.protocol = "modbus-ascii".into();
        }
        assert_eq!(decode(&rows, None).unwrap().outcome, "请求与响应匹配");
        rows[1].bytes = b":01060010002BBE\r\n".to_vec();
        assert_eq!(decode(&rows, None).unwrap().outcome, "写入回显不匹配");
        rows[1].bytes[12] = b'0';
        assert!(decode(&rows, None).unwrap().response.error.is_some());
        rows[1].bytes.clear();
        assert!(decode(&rows, None).unwrap().outcome.contains("不完整"));
        rows[1].transaction = 2;
        assert!(decode(&rows, None).is_err());
    }
}
