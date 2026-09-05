//! Rebuildable metadata index. Raw recording files remain the source of truth.
use crate::recording::{self, Record};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{
    collections::hash_map::DefaultHasher,
    fs::{self, File},
    hash::{Hash, Hasher},
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::State;

#[derive(Clone)]
pub struct RecordingIndex {
    root: PathBuf,
    lock: Arc<Mutex<()>>,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RecordFilter {
    pub query: String,
    pub direction: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedPage {
    pub records: Vec<Record>,
    pub offset: u64,
    pub next_offset: Option<u64>,
    pub total: u64,
    pub warning: Option<String>,
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

impl RecordingIndex {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            lock: Arc::default(),
        }
    }

    fn sync(&self, path: &str) -> Result<(Connection, PathBuf, Option<String>)> {
        let folder = fs::canonicalize(path)?;
        let files = recording::session_files(&folder).map_err(std::io::Error::other)?;
        fs::create_dir_all(&self.root)?;
        let identity = folder.to_string_lossy().into_owned();
        let mut hash = DefaultHasher::new();
        identity.hash(&mut hash);
        let mut db = Connection::open(self.root.join(format!("{:016x}.sqlite3", hash.finish())))?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS metadata (path TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS files (name TEXT PRIMARY KEY, size INTEGER, modified TEXT, scanned INTEGER, damaged INTEGER);
             CREATE TABLE IF NOT EXISTS records (
               id INTEGER PRIMARY KEY, file TEXT NOT NULL, position INTEGER NOT NULL, length INTEGER NOT NULL,
               source TEXT, protocol TEXT, direction TEXT, txn TEXT, search TEXT);
             CREATE INDEX IF NOT EXISTS records_direction ON records(direction, id);
             CREATE INDEX IF NOT EXISTS records_transaction ON records(source, protocol, txn, id);"
        )?;
        let stored: Option<String> = db
            .query_row("SELECT path FROM metadata LIMIT 1", [], |r| r.get(0))
            .ok();
        match stored {
            Some(p) if p != identity => return Err("录制索引路径冲突".into()),
            None => {
                db.execute("INSERT INTO metadata VALUES (?)", [&identity])?;
            }
            _ => {}
        }
        let mut observed = Vec::new();
        let mut rebuild = false;
        for file in &files {
            let name = file
                .file_name()
                .ok_or("无效分卷路径")?
                .to_string_lossy()
                .into_owned();
            let metadata = file.metadata()?;
            let modified = format!("{:?}", metadata.modified()?);
            let old = db
                .query_row(
                    "SELECT size, modified, scanned FROM files WHERE name=?",
                    [&name],
                    |r| {
                        Ok((
                            r.get::<_, u64>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, u64>(2)?,
                        ))
                    },
                )
                .ok();
            if let Some((size, stamp, _)) = &old {
                // Only the last volume may grow. Replaced/truncated/sealed files rebuild the cache.
                if metadata.len() < *size
                    || (metadata.len() == *size && *stamp != modified)
                    || (metadata.len() != *size && Some(file) != files.last())
                {
                    rebuild = true;
                }
            }
            observed.push((name, metadata.len(), modified, old));
        }
        let known: Vec<String> = db
            .prepare("SELECT name FROM files")?
            .query_map([], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        if known
            .iter()
            .any(|name| !observed.iter().any(|f| &f.0 == name))
        {
            rebuild = true;
        }
        let tx = db.transaction()?;
        if rebuild {
            tx.execute_batch("DELETE FROM records; DELETE FROM files;")?;
        }
        let mut pending_tail = false;
        for (name, size, modified, old) in &observed {
            let start = if rebuild {
                0
            } else {
                old.as_ref().map_or(0, |f| f.2)
            };
            if start == *size && !rebuild && old.is_some() {
                continue;
            }
            let mut reader = BufReader::new(File::open(folder.join(name))?);
            reader.seek(SeekFrom::Start(start))?;
            let mut position = start;
            let mut damaged = 0_u64;
            while position < *size {
                let mut line = Vec::new();
                reader
                    .by_ref()
                    .take((size - position).min(1024 * 1024 + 1))
                    .read_until(b'\n', &mut line)?;
                if line.len() > 1024 * 1024 {
                    return Err("录制行超过 1 MiB，文件无效".into());
                }
                if !line.ends_with(b"\n") {
                    pending_tail = true;
                    break;
                }
                match serde_json::from_slice::<Record>(&line) {
                    Ok(r) => {
                        let hex = r
                            .bytes
                            .iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let search = format!(
                            "{} {} {} {} {} {}",
                            r.source, r.protocol, r.direction, r.transaction, r.detail, hex
                        )
                        .to_lowercase();
                        tx.execute("INSERT INTO records(file,position,length,source,protocol,direction,txn,search) VALUES (?,?,?,?,?,?,?,?)",
                            params![name, position, line.len() as u64, r.source, r.protocol, r.direction, r.transaction.to_string(), search])?;
                    }
                    Err(_) => damaged += 1,
                }
                position += line.len() as u64;
            }
            tx.execute("INSERT INTO files VALUES (?,?,?,?,?) ON CONFLICT(name) DO UPDATE SET size=excluded.size, modified=excluded.modified, scanned=excluded.scanned, damaged=files.damaged+excluded.damaged",
                params![name, size, modified, position, damaged])?;
        }
        tx.commit()?;
        let damaged: u64 = db.query_row("SELECT COALESCE(SUM(damaged),0) FROM files", [], |r| {
            r.get(0)
        })?;
        let mut warnings = recording::session_warning(&folder)
            .into_iter()
            .collect::<Vec<_>>();
        let count: u64 = db.query_row("SELECT COUNT(*) FROM records", [], |r| r.get(0))?;
        if let Some(message) = recording::count_warning(&folder, count) {
            warnings.push(message);
        }
        if damaged > 0 {
            warnings.push(format!("已跳过 {damaged} 条损坏记录，录制不完整"));
        }
        if pending_tail {
            warnings.push("存在未完整落盘的尾行，等待后续写入；当前不计入结果".into());
        }
        Ok((
            db,
            folder,
            (!warnings.is_empty()).then(|| warnings.join("；")),
        ))
    }

    pub fn page(
        &self,
        path: &str,
        offset: u64,
        filter: &RecordFilter,
        tail: bool,
    ) -> std::result::Result<IndexedPage, String> {
        let run = || -> Result<IndexedPage> {
            let _guard = self.lock.lock().map_err(|_| "录制索引锁损坏")?;
            let (db, folder, warning) = self.sync(path)?;
            let mut clauses = Vec::new();
            let mut values = Vec::new();
            if !filter.direction.is_empty() {
                clauses.push("direction=?");
                values.push(filter.direction.clone());
            }
            if !filter.query.is_empty() {
                clauses.push("instr(search,?)>0");
                values.push(filter.query.to_lowercase());
            }
            let condition = if clauses.is_empty() {
                String::new()
            } else {
                format!(" WHERE {}", clauses.join(" AND "))
            };
            let total: u64 = db.query_row(
                &format!("SELECT COUNT(*) FROM records{condition}"),
                rusqlite::params_from_iter(values.iter()),
                |r| r.get(0),
            )?;
            let offset = if tail {
                total.saturating_sub(100)
            } else if offset >= total && total > 0 {
                (total - 1) / 100 * 100
            } else {
                offset
            };
            // Tail reads seek backwards through the index instead of skipping every earlier row.
            let suffix = if tail {
                "ORDER BY id DESC LIMIT 100"
            } else {
                values.push(offset.to_string());
                "ORDER BY id LIMIT 100 OFFSET ?"
            };
            let mut statement = db.prepare(&format!(
                "SELECT file,position,length FROM records{condition} {suffix}"
            ))?;
            let locations = statement
                .query_map(rusqlite::params_from_iter(values.iter()), |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })?;
            let mut records = Vec::new();
            for location in locations {
                records.push(read_record(&folder, location?)?);
            }
            if tail {
                records.reverse();
            }
            let end = offset + records.len() as u64;
            Ok(IndexedPage {
                records,
                offset,
                next_offset: (end < total).then_some(end),
                total,
                warning,
            })
        };
        run().map_err(|e| e.to_string())
    }

    pub fn transaction(
        &self,
        path: &str,
        source: &str,
        protocol: &str,
        transaction: u64,
    ) -> std::result::Result<crate::modbus_decode::Transaction, String> {
        let run = || -> Result<crate::modbus_decode::Transaction> {
            let _guard = self.lock.lock().map_err(|_| "录制索引锁损坏")?;
            let (db, folder, warning) = self.sync(path)?;
            let mut stmt = db.prepare("SELECT file,position,length FROM records WHERE source=? AND protocol=? AND txn=? ORDER BY id LIMIT 4097")?;
            let locations = stmt
                .query_map(params![source, protocol, transaction.to_string()], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })?;
            let mut records = Vec::new();
            let mut bytes = 0_u64;
            for location in locations {
                let location: (String, u64, u64) = location?;
                bytes += location.2;
                if bytes > 2 * 1024 * 1024 {
                    return Err("事务记录超过 2 MiB，无法可靠解析".into());
                }
                records.push(read_record(&folder, location)?);
            }
            if records.len() > 4096 {
                return Err("事务记录超过 4096 条，无法可靠解析".into());
            }
            crate::modbus_decode::decode(&records, warning).map_err(Into::into)
        };
        run().map_err(|e| e.to_string())
    }
}

fn read_record(folder: &Path, (name, position, length): (String, u64, u64)) -> Result<Record> {
    if Path::new(&name).file_name().and_then(|s| s.to_str()) != Some(name.as_str())
        || length > 1024 * 1024
    {
        return Err("索引记录无效".into());
    }
    let mut file = File::open(folder.join(name))?;
    file.seek(SeekFrom::Start(position))?;
    let mut bytes = vec![0; length as usize];
    file.read_exact(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[tauri::command]
pub async fn recording_page(
    path: String,
    offset: u64,
    filter: Option<RecordFilter>,
    tail: Option<bool>,
    state: State<'_, RecordingIndex>,
) -> std::result::Result<IndexedPage, String> {
    let index = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        index.page(
            &path,
            offset,
            &filter.unwrap_or_default(),
            tail.unwrap_or(false),
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn recording_modbus_transaction(
    path: String,
    source: String,
    protocol: String,
    transaction: u64,
    state: State<'_, RecordingIndex>,
) -> std::result::Result<crate::modbus_decode::Transaction, String> {
    let index = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        index.transaction(&path, &source, &protocol, transaction)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn record(n: u64) -> Record {
        Record {
            sequence: n,
            timestamp_us: n,
            elapsed_us: n,
            source: "test-port".into(),
            protocol: "modbus-rtu".into(),
            direction: if n % 2 == 0 { "rx" } else { "tx" }.into(),
            transaction: n,
            bytes: vec![1, 3],
            detail: format!("record-{n}"),
        }
    }
    #[test]
    fn global_filter_tail_resume_rotation_and_rebuild() {
        let root = std::env::temp_dir().join(format!("servo-index-{}", recording::timestamp_us()));
        let session = root.join("session");
        fs::create_dir_all(&session).unwrap();
        fs::write(
            session.join("session.json"),
            r#"{"format":"servo-recording","version":1}"#,
        )
        .unwrap();
        let path = session.to_str().unwrap();
        let volume = session.join("records-0001.jsonl");
        let mut file = File::create(&volume).unwrap();
        for n in 1..=250 {
            writeln!(file, "{}", serde_json::to_string(&record(n)).unwrap()).unwrap();
        }
        file.flush().unwrap();
        let index = RecordingIndex::new(root.join("cache"));
        let page = index
            .page(
                path,
                0,
                &RecordFilter {
                    query: "record-248".into(),
                    direction: "rx".into(),
                },
                false,
            )
            .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.records[0].sequence, 248);
        let tail = index.page(path, 0, &Default::default(), true).unwrap();
        assert_eq!(tail.total, 250);
        assert_eq!(tail.offset, 150);
        assert_eq!(tail.records.last().unwrap().sequence, 250);
        // Half a JSON line is not indexed and must be retried after it becomes complete.
        let next = serde_json::to_vec(&record(251)).unwrap();
        file.write_all(&next[..10]).unwrap();
        file.flush().unwrap();
        assert_eq!(
            index
                .page(path, 0, &Default::default(), true)
                .unwrap()
                .total,
            250
        );
        file.write_all(&next[10..]).unwrap();
        file.write_all(b"\n").unwrap();
        file.flush().unwrap();
        drop(file);
        let index = RecordingIndex::new(root.join("cache"));
        assert_eq!(
            index
                .page(path, 0, &Default::default(), true)
                .unwrap()
                .total,
            251
        );
        fs::write(
            session.join("records-0002.jsonl"),
            format!("{}\n", serde_json::to_string(&record(252)).unwrap()),
        )
        .unwrap();
        assert_eq!(
            index
                .page(path, 0, &Default::default(), true)
                .unwrap()
                .total,
            252
        );
        fs::write(
            &volume,
            format!("{}\n", serde_json::to_string(&record(1)).unwrap()),
        )
        .unwrap();
        assert_eq!(
            index
                .page(path, 0, &Default::default(), false)
                .unwrap()
                .total,
            2
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn malformed_lines_are_skipped_without_hiding_integrity_warning() {
        let root =
            std::env::temp_dir().join(format!("servo-index-bad-{}", recording::timestamp_us()));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("session.json"),
            r#"{"format":"servo-recording","version":1}"#,
        )
        .unwrap();
        fs::write(
            root.join("records-0001.jsonl"),
            format!("bad\n{}\n", serde_json::to_string(&record(1)).unwrap()),
        )
        .unwrap();
        let index = RecordingIndex::new(root.join("cache"));
        for _ in 0..2 {
            let page = index
                .page(root.to_str().unwrap(), 0, &Default::default(), false)
                .unwrap();
            assert_eq!(page.total, 1);
            assert!(page.warning.unwrap().contains("1 条损坏"));
        }
        fs::remove_dir_all(root).unwrap();
    }
}
