//! Stateless EtherCAT frame decoding. No heuristic PDO/mailbox identification.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

fn u16le(b: &[u8]) -> u16 {
    u16::from_le_bytes([b[0], b[1]])
}
fn u32le(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecodeConfig {
    #[serde(default)]
    pub mailboxes: Vec<MailboxMap>,
    #[serde(default)]
    pub pdo: Vec<PdoMap>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MailboxMap {
    pub station: u16,
    pub offset: u16,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PdoMap {
    pub name: String,
    pub logical_address: u32,
    pub bit_offset: u8,
    pub bit_length: u8,
    #[serde(default)]
    pub signed: bool,
    #[serde(default = "one")]
    pub scale: f64,
    #[serde(default)]
    pub unit: String,
}
fn one() -> f64 {
    1.0
}
impl DecodeConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.mailboxes.len() > 400 || self.pdo.len() > 512 {
            return Err("映射条目过多".into());
        }
        for p in &self.pdo {
            if p.name.is_empty()
                || p.name.len() > 128
                || p.bit_offset > 7
                || !(1..=64).contains(&p.bit_length)
                || !p.scale.is_finite()
            {
                return Err("PDO 映射名称、位偏移、位宽或缩放无效".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    pub protocol: String,
    pub fields: Value,
    pub datagrams: Vec<Value>,
    pub error: Option<String>,
}
pub fn decode(bytes: &[u8], config: &DecodeConfig) -> Frame {
    let mut frame = Frame {
        protocol: "ethernet".into(),
        fields: json!({}),
        datagrams: vec![],
        error: None,
    };
    if let Err(e) = decode_into(bytes, config, &mut frame) {
        frame.error = Some(e);
    }
    frame
}
fn decode_into(b: &[u8], config: &DecodeConfig, frame: &mut Frame) -> Result<(), String> {
    if b.len() < 14 {
        return Err("Ethernet 头截断".into());
    }
    let mut pos = 14;
    let mut kind = u16::from_be_bytes([b[12], b[13]]);
    let mut vlans = Vec::new();
    while matches!(kind, 0x8100 | 0x88a8) {
        if b.len() < pos + 4 {
            return Err("VLAN 头截断".into());
        }
        vlans.push(u16::from_be_bytes([b[pos], b[pos + 1]]));
        kind = u16::from_be_bytes([b[pos + 2], b[pos + 3]]);
        pos += 4;
    }
    frame.fields = json!({"destination":hex(&b[..6]), "source":hex(&b[6..12]), "etherType":kind, "vlanTci":vlans});
    if kind != 0x88a4 {
        return Ok(());
    }
    frame.protocol = "ethercat".into();
    if b.len() < pos + 2 {
        return Err("EtherCAT 帧头截断".into());
    }
    let header = u16le(&b[pos..]);
    pos += 2;
    let length = (header & 0x7ff) as usize;
    let end = pos + length;
    frame.fields["ethercatType"] = json!(header >> 12);
    frame.fields["length"] = json!(length);
    if end > b.len() {
        return Err("EtherCAT 声明长度超出捕获长度".into());
    }
    if header >> 12 != 1 {
        frame.fields["payload"] = json!(hex(&b[pos..end]));
        return Ok(());
    }
    if header & 0x800 != 0 {
        return Err("EtherCAT 保留位非零".into());
    }
    while pos < end {
        if end - pos < 12 {
            return Err("Datagram 头或 WKC 截断".into());
        }
        let command = b[pos];
        let index = b[pos + 1];
        let address = u32le(&b[pos + 2..]);
        let flags = u16le(&b[pos + 6..]);
        let count = (flags & 0x7ff) as usize;
        let data_start = pos + 10;
        let data_end = data_start + count;
        if data_end + 2 > end {
            return Err("Datagram 数据长度越界".into());
        }
        let data = &b[data_start..data_end];
        let wkc = u16le(&b[data_end..]);
        let name = [
            "NOP", "APRD", "APWR", "APRW", "FPRD", "FPWR", "FPRW", "BRD", "BWR", "BRW", "LRD",
            "LWR", "LRW", "ARMW", "FRMW",
        ]
        .get(command as usize)
        .copied()
        .unwrap_or("UNKNOWN");
        let mut d = json!({"command":name, "commandCode":command, "index":index, "address":address,
            "adp":u16le(&b[pos+2..]), "ado":u16le(&b[pos+4..]), "length":count,
            "circulating":flags & 0x4000 != 0, "more":flags & 0x8000 != 0,
            "irq":u16le(&b[pos+8..]), "wkc":wkc, "data":hex(data), "offset":pos});
        if matches!(command, 4..=6 | 14) {
            let station = u16le(&b[pos + 2..]);
            let offset = u16le(&b[pos + 4..]);
            if config
                .mailboxes
                .iter()
                .any(|m| m.station == station && m.offset == offset)
            {
                d["mailbox"] = match mailbox(data) {
                    Ok(v) => v,
                    Err(e) => json!({"error":e}),
                };
            }
            if offset == 0x0130 && data.len() >= 2 {
                d["alState"] = json!(u16le(data));
            }
            if offset == 0x0134 && data.len() >= 2 {
                d["alStatusCode"] = json!(u16le(data));
            }
            if offset == 0x0910 && data.len() >= 8 {
                d["dcSystemTimeNs"] =
                    json!(u64::from_le_bytes(data[..8].try_into().unwrap()).to_string());
            }
        }
        if matches!(command, 10..=12) {
            let mut values = vec![];
            for map in &config.pdo {
                let Some(offset) = map.logical_address.checked_sub(address) else {
                    continue;
                };
                let bit = offset as usize * 8 + map.bit_offset as usize;
                if bit + map.bit_length as usize > data.len() * 8 {
                    continue;
                }
                let mut raw = 0u64;
                for i in 0..map.bit_length as usize {
                    raw |= (((data[(bit + i) / 8] >> ((bit + i) % 8)) & 1) as u64) << i;
                }
                let signed = ((raw << (64 - map.bit_length)) as i64) >> (64 - map.bit_length);
                let value = if map.signed {
                    signed as f64
                } else {
                    raw as f64
                } * map.scale;
                values.push(json!({"name":map.name, "raw":if map.signed { signed.to_string() } else { raw.to_string() }, "value":value, "unit":map.unit}));
            }
            d["pdo"] = json!(values);
        }
        frame.datagrams.push(d);
        pos = data_end + 2;
        if flags & 0x8000 == 0 {
            if pos != end {
                return Err("最后一个 Datagram 后仍有声明负载".into());
            }
            break;
        }
        if pos == end {
            return Err("Datagram more 标志后缺少下一报文".into());
        }
    }
    Ok(())
}
pub fn hex(b: &[u8]) -> String {
    b.iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
fn mailbox(data: &[u8]) -> Result<Value, String> {
    if data.len() < 6 {
        return Err("邮箱头截断".into());
    }
    let len = u16le(data) as usize;
    if len > data.len() - 6 {
        return Err("邮箱负载截断".into());
    }
    let kind = data[5] & 0x0f;
    let payload = &data[6..6 + len];
    let name = match kind {
        0 => "Error",
        1 => "AoE",
        2 => "EoE",
        3 => "CoE",
        4 => "FoE",
        5 => "SoE",
        15 => "VoE",
        _ => "Unknown",
    };
    let mut v = json!({"protocol":name,"length":len,"address":u16le(&data[2..]),"channel":data[4]&0x3f,"priority":data[4]>>6,"counter":(data[5]>>4)&7,"data":hex(payload)});
    match kind {
        3 if len >= 2 => {
            let header = u16le(payload);
            let service = header >> 12;
            v["service"] = json!(service);
            v["number"] = json!(header & 0x1ff);
            if matches!(service, 2 | 3) && len >= 3 {
                let command = payload[2] >> 5;
                v["sdoCommand"] = json!(command);
                v["sdoHeader"] = json!(payload[2]);
                let indexed = command == 4
                    || (service == 2 && matches!(command, 1 | 2))
                    || (service == 3 && matches!(command, 2 | 3));
                if indexed && len >= 6 {
                    v["index"] = json!(u16le(&payload[3..]));
                    v["subIndex"] = json!(payload[5]);
                    if command == 4 && len >= 10 {
                        v["abortCode"] = json!(u32le(&payload[6..]));
                    }
                    if ((service == 2 && command == 1) || (service == 3 && command == 2))
                        && len >= 10
                    {
                        let expedited = payload[2] & 2 != 0;
                        v["expedited"] = json!(expedited);
                        if expedited {
                            let n = if payload[2] & 1 != 0 {
                                4 - ((payload[2] >> 2) & 3) as usize
                            } else {
                                4
                            };
                            v["valueBytes"] = json!(hex(&payload[6..6 + n]));
                        } else if payload[2] & 1 != 0 {
                            v["totalSize"] = json!(u32le(&payload[6..]));
                        }
                    }
                } else {
                    v["segment"] = json!({"toggle":payload[2]&0x10 != 0,"last":payload[2]&1 != 0,"raw":hex(&payload[3..])});
                }
            }
        }
        4 if len >= 6 => {
            v["opcode"] = json!(payload[0]);
            v["packetOrErrorOrPassword"] = json!(u32le(&payload[2..]));
        }
        5 if len >= 4 => {
            v["opcode"] = json!(payload[0] & 7);
            v["incomplete"] = json!(payload[0] & 8 != 0);
            v["error"] = json!(payload[0] & 0x10 != 0);
            v["drive"] = json!(payload[0] >> 5);
            v["elements"] = json!(payload[1]);
            v["idnOrFragments"] = json!(u16le(&payload[2..]));
        }
        2 if len >= 4 => {
            v["type"] = json!(payload[0] & 15);
            v["lastFragment"] = json!(payload[1] & 1 != 0);
            v["fragmentInfo"] = json!(u16le(&payload[2..]));
        }
        1 if len >= 32 => {
            v["targetNetId"] = json!(hex(&payload[..6]));
            v["targetPort"] = json!(u16le(&payload[6..]));
            v["sourceNetId"] = json!(hex(&payload[8..14]));
            v["commandId"] = json!(u16le(&payload[16..]));
            v["stateFlags"] = json!(u16le(&payload[18..]));
            v["dataLength"] = json!(u32le(&payload[20..]));
            v["errorCode"] = json!(u32le(&payload[24..]));
            v["invokeId"] = json!(u32le(&payload[28..]));
        }
        _ => {}
    }
    Ok(v)
}
#[tauri::command]
pub fn decode_ethercat(bytes: Vec<u8>, config: DecodeConfig) -> Result<Frame, String> {
    if bytes.len() > 65536 {
        return Err("单帧超出 64 KiB".into());
    }
    config.validate()?;
    Ok(decode(&bytes, &config))
}
#[cfg(test)]
mod tests {
    use super::*;
    pub fn sample() -> Vec<u8> {
        let mut b = vec![0; 12];
        b.extend([
            0x88, 0xa4, 14, 0x10, 0x0c, 7, 0, 0x10, 0, 0, 2, 0, 0, 0, 0x34, 0x12, 1, 0,
        ]);
        b
    }
    #[test]
    fn parses_and_rejects_truncations() {
        let b = sample();
        let f = decode(&b, &DecodeConfig::default());
        assert!(f.error.is_none());
        assert_eq!(f.datagrams[0]["command"], "LRW");
        assert_eq!(f.datagrams[0]["wkc"], 1);
        for n in 14..b.len() {
            assert!(decode(&b[..n], &DecodeConfig::default()).error.is_some());
        }
    }
    #[test]
    fn signed_pdo_and_no_mailbox_guessing() {
        let mut c = DecodeConfig::default();
        c.pdo.push(PdoMap {
            name: "test".into(),
            logical_address: 4096,
            bit_offset: 0,
            bit_length: 16,
            signed: true,
            scale: 0.5,
            unit: "".into(),
        });
        let f = decode(&sample(), &c);
        assert_eq!(f.datagrams[0]["pdo"][0]["value"], 2330.0);
        assert!(f.datagrams[0].get("mailbox").is_none());
    }
    #[test]
    fn multiple_and_vlan() {
        let mut b = sample();
        let dg = b[16..].to_vec();
        b[12..14].copy_from_slice(&[0x81, 0]);
        b.splice(14..14, [0, 1, 0x88, 0xa4]);
        b[18] = 28;
        b[27] |= 0x80;
        b.extend(dg);
        let f = decode(&b, &DecodeConfig::default());
        assert!(f.error.is_none(), "{:?}", f.error);
        assert_eq!(f.datagrams.len(), 2);
    }
    #[test]
    fn mailbox_abort() {
        let mut b = vec![10, 0, 0, 0, 0, 0x13, 0, 0x30, 0x80, 0, 0x20, 1];
        b.extend(0x06020000u32.to_le_bytes());
        assert_eq!(mailbox(&b).unwrap()["abortCode"], 0x06020000u32);
    }
}
