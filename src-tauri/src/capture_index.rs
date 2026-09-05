//! Convert selected PCAP volumes into a rebuildable recording cache for the common index.
use crate::{pcap_file::CaptureReader, recording, recording_library::save_json};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    fs::File,
    hash::{Hash, Hasher},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

fn sources(path: &Path) -> Result<Vec<PathBuf>, String> {
    if path.is_file() {
        return Ok(vec![path.into()]);
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
        let p = entry.map_err(|e| e.to_string())?.path();
        if p.is_file()
            && p.extension().is_some_and(|e| {
                matches!(
                    e.to_string_lossy().to_lowercase().as_str(),
                    "pcap" | "pcapng"
                )
            })
        {
            files.push(p);
        }
    }
    files.sort();
    if files.is_empty() {
        return Err("目录中没有 PCAP / PCAPNG 分卷".into());
    }
    Ok(files)
}
fn signature(files: &[PathBuf]) -> Result<String, String> {
    let mut result = String::new();
    for p in files {
        let m = p.metadata().map_err(|e| e.to_string())?;
        result.push_str(&format!(
            "{}|{}|{:?}\n",
            p.display(),
            m.len(),
            m.modified().map_err(|e| e.to_string())?
        ));
    }
    Ok(result)
}
pub fn materialize(path: &Path, root: &Path) -> Result<PathBuf, String> {
    let files = sources(path)?;
    let fingerprint_for = |mut observed: Vec<PathBuf>| {
        if path.is_dir() && path.join("summary.json").is_file() {
            observed.push(path.join("summary.json"));
        }
        signature(&observed)
    };
    let fingerprint = fingerprint_for(files.clone())?;
    let mut hash = DefaultHasher::new();
    path.hash(&mut hash);
    let folder = root.join(format!("capture-{:016x}", hash.finish()));
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let manifest = folder.join("source.json");
    if fs::read_to_string(&manifest).ok().as_deref() == Some(&fingerprint)
        && folder.join("summary.json").is_file()
    {
        return Ok(folder);
    }
    // A failed rebuild must never be treated as a completed cache on the next request.
    if manifest.exists() {
        fs::remove_file(&manifest).map_err(|e| e.to_string())?;
    }
    let mut output =
        BufWriter::new(File::create(folder.join("records.tmp")).map_err(|e| e.to_string())?);
    let mut count = 0;
    let mut bytes = 0;
    let mut warnings = Vec::new();
    for file in &files {
        let mut reader = match CaptureReader::open(file) {
            Ok(reader) => reader,
            Err(e) => {
                warnings.push(format!("{}：{e}", file.display()));
                continue;
            }
        };
        loop {
            let mut record = match reader.next_record() {
                Ok(Some(record)) => record,
                Ok(None) => break,
                Err(e) => {
                    warnings.push(format!("{}：{e}", file.display()));
                    break;
                }
            };
            if record.bytes.len() > 128 * 1024 {
                warnings.push(format!(
                    "{}：单帧超过 128 KiB，该帧及后续记录未导入",
                    file.display()
                ));
                break;
            }
            count += 1;
            record.sequence = count;
            record.source = format!(
                "{} / {}",
                file.file_name().unwrap().to_string_lossy(),
                record.source
            );
            let mut line = serde_json::to_vec(&record).map_err(|e| e.to_string())?;
            line.push(b'\n');
            output.write_all(&line).map_err(|e| e.to_string())?;
            bytes += line.len() as u64;
        }
    }
    if path.is_dir() && path.join("summary.json").is_file() {
        match fs::read(path.join("summary.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        {
            Some(s)
                if s["active"] == true
                    || s["error"].is_string()
                    || s["driverDropped"].as_u64().unwrap_or(0) > 0
                    || s["packets"].as_u64().is_some_and(|n| n != count) =>
            {
                warnings.push(format!(
                    "源捕获摘要提示未结束、错误、驱动丢包或包数不符（实际可读 {count}）：{s}"
                ))
            }
            None => warnings.push("源捕获摘要损坏，不能确认完整性".into()),
            _ => {}
        }
    }
    output
        .flush()
        .and_then(|()| output.get_ref().sync_all())
        .map_err(|e| e.to_string())?;
    drop(output);
    if fingerprint != fingerprint_for(sources(path)?)? {
        return Err("源文件在索引期间发生变化，请停止捕获后重新打开".into());
    }
    fs::rename(
        folder.join("records.tmp"),
        folder.join("records-0001.jsonl"),
    )
    .map_err(|e| e.to_string())?;
    save_json(
        &folder.join("session.json"),
        &serde_json::json!({"format":"servo-recording", "version":1, "source":path, "context":{}, "imported":true}),
    )?;
    save_json(
        &folder.join("summary.json"),
        &recording::RecordingStatus {
            path: folder.to_string_lossy().into(),
            accepted: count,
            written: count,
            bytes,
            error: (!warnings.is_empty()).then(|| warnings.join("；")),
            ..Default::default()
        },
    )?;
    fs::write(manifest, fingerprint).map_err(|e| e.to_string())?;
    Ok(folder)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        pcap_file::PcapWriter,
        recording_index::{RecordFilter, RecordingIndex},
    };
    #[test]
    fn cross_volume_filter_time_and_truncation() {
        let root =
            std::env::temp_dir().join(format!("servo-pcap-index-{}", recording::timestamp_us()));
        let source = root.join("source");
        fs::create_dir_all(&source).unwrap();
        for n in 1..=2 {
            let mut w = PcapWriter::create(&source.join(format!("frames-{n}.pcapng"))).unwrap();
            w.packet("test", &[0; 14], n * 100, 14).unwrap();
            w.finish().unwrap();
        }
        let index = RecordingIndex::new(root.join("cache"));
        let p = index
            .page(
                source.to_str().unwrap(),
                0,
                &RecordFilter {
                    from_us: Some(150),
                    ..Default::default()
                },
                false,
            )
            .unwrap();
        assert_eq!(p.total, 1);
        assert_eq!(p.records[0].timestamp_us, 200);
        fs::write(
            source.join("summary.json"),
            r#"{"active":false,"packets":3,"error":null,"driverDropped":0}"#,
        )
        .unwrap();
        let p = index
            .page(source.to_str().unwrap(), 0, &Default::default(), false)
            .unwrap();
        assert_eq!(p.total, 2);
        assert!(p.warning.unwrap().contains("包数不符"));
        fs::write(source.join("frames-3.pcap"), [0, 1, 2]).unwrap();
        let p = index
            .page(source.to_str().unwrap(), 0, &Default::default(), false)
            .unwrap();
        assert_eq!(p.total, 2);
        assert!(p.warning.unwrap().contains("frames-3"));
        fs::remove_dir_all(root).unwrap();
    }
}
