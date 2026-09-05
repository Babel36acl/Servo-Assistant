//! Streaming PCAP/PCAPNG reader; bounded allocation even for corrupt input.
use crate::recording::{Record, RecordPage};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufReader, BufWriter, Read, Write},
    path::Path,
};
fn word(b: &[u8], little: bool) -> u32 {
    let a = b[..4].try_into().unwrap();
    if little {
        u32::from_le_bytes(a)
    } else {
        u32::from_be_bytes(a)
    }
}
fn short(b: &[u8], little: bool) -> u16 {
    let a = b[..2].try_into().unwrap();
    if little {
        u16::from_le_bytes(a)
    } else {
        u16::from_be_bytes(a)
    }
}
fn exact(r: &mut impl Read, n: usize) -> Result<Vec<u8>, String> {
    if n > 16 * 1024 * 1024 {
        return Err("捕获块超过 16 MiB 上限".into());
    }
    let mut b = vec![0; n];
    r.read_exact(&mut b)
        .map_err(|e| format!("捕获文件截断：{e}"))?;
    Ok(b)
}
fn header(r: &mut impl Read) -> Result<Option<[u8; 8]>, String> {
    let mut b = [0; 8];
    match r.read(&mut b[..1]) {
        Ok(0) => Ok(None),
        Ok(_) => {
            r.read_exact(&mut b[1..]).map_err(|e| e.to_string())?;
            Ok(Some(b))
        }
        Err(e) => Err(e.to_string()),
    }
}
#[derive(Clone)]
struct Interface {
    link: u16,
    resolution: u8,
    snaplen: u32,
    offset: i64,
}
pub struct CaptureReader {
    reader: BufReader<File>,
    little: bool,
    ng: bool,
    nano: bool,
    link: u16,
    interfaces: Vec<Interface>,
    sequence: u64,
    section: u64,
}
impl CaptureReader {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut reader = BufReader::new(File::open(path).map_err(|e| e.to_string())?);
        let magic = exact(&mut reader, 4)?;
        let ng = magic == [0x0a, 0x0d, 0x0d, 0x0a];
        let (little, nano) = match magic.as_slice() {
            [0xd4, 0xc3, 0xb2, 0xa1] => (true, false),
            [0xa1, 0xb2, 0xc3, 0xd4] => (false, false),
            [0x4d, 0x3c, 0xb2, 0xa1] => (true, true),
            [0xa1, 0xb2, 0x3c, 0x4d] => (false, true),
            _ if ng => (true, false),
            _ => return Err("不支持的捕获格式，请提供 PCAP 或 PCAPNG".into()),
        };
        let mut result = Self {
            reader,
            little,
            ng,
            nano,
            link: 1,
            interfaces: vec![],
            sequence: 0,
            section: 0,
        };
        if ng {
            let len = exact(&mut result.reader, 4)?;
            result.section(&len)?;
        } else {
            let h = exact(&mut result.reader, 20)?;
            if short(&h, little) != 2 || short(&h[2..], little) != 4 {
                return Err("PCAP 版本必须为 2.4".into());
            }
            result.link = (word(&h[16..], little) & 0xffff) as u16;
        }
        Ok(result)
    }
    fn section(&mut self, length: &[u8]) -> Result<(), String> {
        let bom = exact(&mut self.reader, 4)?;
        self.little = match bom.as_slice() {
            [0x4d, 0x3c, 0x2b, 0x1a] => true,
            [0x1a, 0x2b, 0x3c, 0x4d] => false,
            _ => return Err("PCAPNG 字节序标记无效".into()),
        };
        let len = word(length, self.little) as usize;
        if len < 28 || !len.is_multiple_of(4) {
            return Err("PCAPNG Section 长度无效".into());
        }
        let rest = exact(&mut self.reader, len - 12)?;
        if word(&rest[rest.len() - 4..], self.little) as usize != len
            || short(&rest, self.little) != 1
        {
            return Err("PCAPNG Section 尾长或版本无效".into());
        }
        self.interfaces.clear();
        self.section += 1;
        Ok(())
    }
    pub fn next_record(&mut self) -> Result<Option<Record>, String> {
        loop {
            let Some(h) = header(&mut self.reader)? else {
                return Ok(None);
            };
            if !self.ng {
                let rest = exact(&mut self.reader, 8)?;
                let cap = word(&rest, self.little) as usize;
                let original = word(&rest[4..], self.little);
                if cap > original as usize {
                    return Err("PCAP 捕获长度大于原始长度".into());
                }
                let data = exact(&mut self.reader, cap)?;
                let frac = word(&h[4..], self.little) as u64;
                let stamp = word(&h, self.little) as u64 * 1_000_000
                    + if self.nano { frac / 1000 } else { frac };
                return Ok(Some(self.record(
                    data,
                    stamp,
                    0,
                    self.link,
                    format!(
                        "originalLength={original}; source timestamp precision={}",
                        if self.nano {
                            "ns (display truncated to us)"
                        } else {
                            "us"
                        }
                    ),
                )));
            }
            if h[..4] == [0x0a, 0x0d, 0x0d, 0x0a] {
                self.section(&h[4..])?;
                continue;
            }
            let kind = word(&h, self.little);
            let len = word(&h[4..], self.little) as usize;
            if len < 12 || !len.is_multiple_of(4) {
                return Err("PCAPNG 块长无效".into());
            }
            let body = exact(&mut self.reader, len - 8)?;
            if word(&body[body.len() - 4..], self.little) as usize != len {
                return Err("PCAPNG 块首尾长度不匹配".into());
            }
            let b = &body[..body.len() - 4];
            match kind {
                1 => {
                    if b.len() < 8 {
                        return Err("Interface 块截断".into());
                    }
                    let mut i = Interface {
                        link: short(b, self.little),
                        resolution: 6,
                        snaplen: word(&b[4..], self.little),
                        offset: 0,
                    };
                    let mut p = 8;
                    while p + 4 <= b.len() {
                        let code = short(&b[p..], self.little);
                        let size = short(&b[p + 2..], self.little) as usize;
                        p += 4;
                        if code == 0 {
                            break;
                        }
                        if p + size > b.len() {
                            return Err("Interface 选项截断".into());
                        }
                        if code == 9 && size == 1 {
                            i.resolution = b[p];
                        }
                        if code == 14 && size == 8 {
                            let a = b[p..p + 8].try_into().unwrap();
                            i.offset = if self.little {
                                i64::from_le_bytes(a)
                            } else {
                                i64::from_be_bytes(a)
                            };
                        }
                        p += (size + 3) & !3;
                    }
                    if self.interfaces.len() >= 4096 {
                        return Err("PCAPNG 接口数量过多".into());
                    }
                    self.interfaces.push(i);
                }
                6 => {
                    if b.len() < 20 {
                        return Err("Enhanced Packet 块截断".into());
                    }
                    let id = word(b, self.little);
                    let i = self
                        .interfaces
                        .get(id as usize)
                        .ok_or("数据包引用不存在的接口")?
                        .clone();
                    let ticks = ((word(&b[4..], self.little) as u64) << 32)
                        | word(&b[8..], self.little) as u64;
                    let cap = word(&b[12..], self.little) as usize;
                    let original = word(&b[16..], self.little);
                    if cap > original as usize || cap > b.len() - 20 {
                        return Err("Enhanced Packet 数据长度无效".into());
                    }
                    let divisor = if i.resolution & 0x80 != 0 {
                        2u128.checked_pow((i.resolution & 127) as u32)
                    } else {
                        10u128.checked_pow(i.resolution as u32)
                    }
                    .ok_or("时间戳分辨率超出支持范围")?;
                    let micros = (ticks as u128 * 1_000_000 / divisor) as i128
                        + i.offset as i128 * 1_000_000;
                    let stamp = u64::try_from(micros).map_err(|_| "时间戳越界")?;
                    return Ok(Some(self.record(
                        b[20..20 + cap].to_vec(),
                        stamp,
                        id,
                        i.link,
                        format!(
                            "originalLength={original}; timestampResolution={}",
                            i.resolution
                        ),
                    )));
                }
                3 => {
                    if b.len() < 4 {
                        return Err("Simple Packet 块截断".into());
                    }
                    let i = self.interfaces.first().ok_or("缺少接口块")?.clone();
                    let original = word(b, self.little) as usize;
                    let cap = if i.snaplen == 0 {
                        original
                    } else {
                        original.min(i.snaplen as usize)
                    };
                    if (cap + 3) & !3 != b.len() - 4 {
                        return Err("Simple Packet 捕获长度或填充无效".into());
                    }
                    return Ok(Some(self.record(
                        b[4..4 + cap].to_vec(),
                        0,
                        0,
                        i.link,
                        format!("originalLength={original}; timestamp unavailable"),
                    )));
                }
                2 => return Err(
                    "旧式 PCAPNG Packet Block 暂不支持，请使用 Wireshark 转存为 Enhanced Packet"
                        .into(),
                ),
                _ => {}
            }
        }
    }
    fn record(
        &mut self,
        bytes: Vec<u8>,
        stamp: u64,
        interface: u32,
        link: u16,
        detail: String,
    ) -> Record {
        self.sequence += 1;
        Record {
            sequence: self.sequence,
            timestamp_us: stamp,
            elapsed_us: 0,
            source: format!("section:{} interface:{interface} link:{link}", self.section),
            protocol: if link == 1 {
                "ethernet".into()
            } else {
                format!("linktype-{link}")
            },
            direction: "unknown".into(),
            transaction: 0,
            bytes,
            detail,
        }
    }
}
#[tauri::command]
pub async fn capture_file_page(path: String, offset: u64) -> Result<RecordPage, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut reader = CaptureReader::open(Path::new(&path))?;
        let mut records = vec![];
        for _ in 0..offset {
            if reader.next_record()?.is_none() {
                return Ok(RecordPage {
                    records,
                    next_offset: None,
                    warning: None,
                });
            }
        }
        while records.len() < 100 {
            match reader.next_record() {
                Ok(Some(r)) => records.push(r),
                Ok(None) => {
                    return Ok(RecordPage {
                        records,
                        next_offset: None,
                        warning: None,
                    })
                }
                Err(e) => {
                    return Ok(RecordPage {
                        records,
                        next_offset: None,
                        warning: Some(e),
                    })
                }
            }
        }
        let (next, warning) = match reader.next_record() {
            Ok(Some(_)) => (Some(offset + 100), None),
            Ok(None) => (None, None),
            Err(e) => (None, Some(e)),
        };
        Ok(RecordPage {
            records,
            next_offset: next,
            warning,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
fn block(writer: &mut impl Write, kind: u32, body: &[u8]) -> Result<(), String> {
    let len = (body.len() + 12) as u32;
    writer
        .write_all(&kind.to_le_bytes())
        .and_then(|_| writer.write_all(&len.to_le_bytes()))
        .and_then(|_| writer.write_all(body))
        .and_then(|_| writer.write_all(&len.to_le_bytes()))
        .map_err(|e| e.to_string())
}
pub struct PcapWriter {
    writer: BufWriter<File>,
    interfaces: HashMap<String, u32>,
}
impl PcapWriter {
    pub fn create(path: &Path) -> Result<Self, String> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        let mut writer = BufWriter::new(file);
        let mut b = Vec::new();
        b.extend(0x1a2b3c4du32.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(0u16.to_le_bytes());
        b.extend((-1i64).to_le_bytes());
        block(&mut writer, 0x0a0d0d0a, &b)?;
        Ok(Self {
            writer,
            interfaces: HashMap::new(),
        })
    }
    pub fn packet(
        &mut self,
        source: &str,
        data: &[u8],
        stamp: u64,
        original: u32,
    ) -> Result<(), String> {
        let id = if let Some(id) = self.interfaces.get(source) {
            *id
        } else {
            let id = self.interfaces.len() as u32;
            let mut b = vec![1, 0, 0, 0];
            b.extend(65536u32.to_le_bytes());
            let name = source.as_bytes();
            let name = &name[..name.len().min(1024)];
            b.extend(2u16.to_le_bytes());
            b.extend((name.len() as u16).to_le_bytes());
            b.extend(name);
            while b.len() % 4 != 0 {
                b.push(0);
            }
            b.extend([0, 0, 0, 0]);
            block(&mut self.writer, 1, &b)?;
            self.interfaces.insert(source.into(), id);
            id
        };
        let mut b = vec![];
        b.extend(id.to_le_bytes());
        b.extend(((stamp >> 32) as u32).to_le_bytes());
        b.extend((stamp as u32).to_le_bytes());
        b.extend((data.len() as u32).to_le_bytes());
        b.extend(original.to_le_bytes());
        b.extend(data);
        while b.len() % 4 != 0 {
            b.push(0);
        }
        block(&mut self.writer, 6, &b)
    }
    pub fn flush(&mut self) -> Result<(), String> {
        self.writer.flush().map_err(|e| e.to_string())
    }
    pub fn finish(mut self) -> Result<(), String> {
        self.flush()?;
        self.writer.get_ref().sync_all().map_err(|e| e.to_string())
    }
}
#[derive(serde::Serialize)]
pub struct ExportResult {
    pub path: String,
    pub warning: Option<String>,
}
#[tauri::command]
pub async fn export_recording_pcap(path: String) -> Result<ExportResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let output = Path::new(&path).join(format!(
            "ethernet-{}.pcapng",
            crate::recording::timestamp_us()
        ));
        // Validate the source before creating the destination.
        let mut page = crate::recording::read_page(&path, 0)?;
        let mut writer = PcapWriter::create(&output)?;
        let mut warning = None;
        loop {
            if let Some(e) = page.warning {
                warning = Some(e);
            }
            for r in page.records {
                if r.protocol == "ethernet" && r.direction != "event" {
                    writer.packet(&r.source, &r.bytes, r.timestamp_us, r.bytes.len() as u32)?;
                }
            }
            match page.next_offset {
                Some(n) => page = crate::recording::read_page(&path, n)?,
                None => break,
            }
        }
        writer.finish()?;
        Ok(ExportResult {
            path: output.to_string_lossy().into(),
            warning,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_pcap_byte_orders_and_nanoseconds() {
        for little in [false, true] {
            let path = std::env::temp_dir().join(format!(
                "servo-legacy-{}.pcap",
                crate::recording::timestamp_us()
            ));
            let mut bytes = if little {
                vec![0x4d, 0x3c, 0xb2, 0xa1]
            } else {
                vec![0xa1, 0xb2, 0x3c, 0x4d]
            };
            for n in [2u16, 4] {
                bytes.extend(if little {
                    n.to_le_bytes()
                } else {
                    n.to_be_bytes()
                });
            }
            for n in [0u32, 0, 65536, 1, 12, 345678901, 3, 7] {
                bytes.extend(if little {
                    n.to_le_bytes()
                } else {
                    n.to_be_bytes()
                });
            }
            bytes.extend([1, 2, 3]);
            std::fs::write(&path, bytes).unwrap();
            let mut reader = CaptureReader::open(&path).unwrap();
            let r = reader.next_record().unwrap().unwrap();
            assert_eq!(r.timestamp_us, 12_345_678);
            assert_eq!(r.bytes, [1, 2, 3]);
            assert!(r.detail.contains("originalLength=7"));
            drop(reader);
            std::fs::remove_file(path).unwrap();
        }
    }
    #[test]
    fn refuses_oversized_section_without_allocating_it() {
        let path = std::env::temp_dir().join(format!(
            "servo-bad-pcap-{}.pcapng",
            crate::recording::timestamp_us()
        ));
        let mut b = vec![0x0a, 0x0d, 0x0d, 0x0a];
        b.extend(0xfffffffcu32.to_le_bytes());
        b.extend(0x1a2b3c4du32.to_le_bytes());
        std::fs::write(&path, b).unwrap();
        assert!(CaptureReader::open(&path).err().unwrap().contains("16 MiB"));
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn pcapng_round_trip_multi_interface() {
        let path = std::env::temp_dir().join(format!(
            "servo-pcap-{}.pcapng",
            crate::recording::timestamp_us()
        ));
        let mut w = PcapWriter::create(&path).unwrap();
        w.packet("a", &[1, 2, 3], 1234567, 3).unwrap();
        w.packet("b", &[4], 9876543, 1).unwrap();
        w.finish().unwrap();
        let mut r = CaptureReader::open(&path).unwrap();
        assert_eq!(r.next_record().unwrap().unwrap().timestamp_us, 1234567);
        let b = r.next_record().unwrap().unwrap();
        assert!(b.source.contains("interface:1"));
        assert_eq!(b.bytes, [4]);
        assert!(r.next_record().unwrap().is_none());
        drop(r);
        std::fs::remove_file(path).unwrap();
    }
}
