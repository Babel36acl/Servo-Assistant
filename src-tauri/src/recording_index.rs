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
    pub source: String,
    pub protocol: String,
    pub from_us: Option<u64>,
    pub to_us: Option<u64>,
    pub slave: Option<u8>,
    pub function: Option<u8>,
    pub address: Option<u16>,
    pub outcome: String,
    pub sequence: Option<u64>,
}
impl RecordFilter {
    fn condition(&self) -> std::result::Result<(String, Vec<String>), String> {
        if self.from_us.zip(self.to_us).is_some_and(|(a, b)| a > b) {
            return Err("开始时间不能晚于结束时间".into());
        }
        let mut clauses = Vec::new();
        let mut values = Vec::new();
        for (field, value) in [
            ("direction", &self.direction),
            ("source", &self.source),
            ("protocol", &self.protocol),
        ] {
            if !value.is_empty() {
                clauses.push(format!("records.{field}=?"));
                values.push(value.clone());
            }
        }
        for (field, op, value) in [
            ("stamp", ">=", self.from_us),
            ("stamp", "<=", self.to_us),
            ("seq", "=", self.sequence),
        ] {
            if let Some(value) = value {
                clauses.push(format!("records.{field}{op}CAST(? AS INTEGER)"));
                values.push(value.to_string());
            }
        }
        if !self.query.is_empty() {
            clauses.push("instr(search,?)>0".into());
            values.push(self.query.to_lowercase());
        }
        let mut transaction = Vec::new();
        for (field, value) in [
            ("slave", self.slave.map(u64::from)),
            ("function", self.function.map(u64::from)),
        ] {
            if let Some(value) = value {
                transaction.push(format!("t.{field}=CAST(? AS INTEGER)"));
                values.push(value.to_string());
            }
        }
        if let Some(address) = self.address {
            transaction.push("t.address<=CAST(? AS INTEGER) AND t.address+COALESCE(t.count,1)>CAST(? AS INTEGER)".into());
            values.extend([address.to_string(), address.to_string()]);
        }
        if !self.outcome.is_empty() {
            if !["success", "failure", "incomplete"].contains(&self.outcome.as_str()) {
                return Err("事务结果筛选无效".into());
            }
            transaction.push("t.outcome=?".into());
            values.push(self.outcome.clone());
        }
        if !transaction.is_empty() {
            clauses.push(format!("EXISTS(SELECT 1 FROM transactions t WHERE t.source=records.source AND t.protocol=records.protocol AND t.txn=records.txn AND {})", transaction.join(" AND ")));
        }
        Ok((
            if clauses.is_empty() {
                String::new()
            } else {
                format!(" WHERE {}", clauses.join(" AND "))
            },
            values,
        ))
    }
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
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub first_us: Option<u64>,
    pub last_us: Option<u64>,
    pub count: u64,
    pub channels: Vec<Channel>,
    pub warning: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub source: String,
    pub protocol: String,
    pub slave: u8,
    pub first_address: u16,
    pub last_address: u16,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct History {
    pub points: Vec<crate::recording_analysis::Point>,
    pub total: u64,
    pub offset: u64,
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
        let original = fs::canonicalize(path)?;
        let folder = if original.join("session.json").is_file() {
            original
        } else {
            crate::capture_index::materialize(&original, &self.root)
                .map_err(std::io::Error::other)?
        };
        let files = recording::session_files(&folder).map_err(std::io::Error::other)?;
        fs::create_dir_all(&self.root)?;
        let identity = folder.to_string_lossy().into_owned();
        let mut hash = DefaultHasher::new();
        identity.hash(&mut hash);
        let mut db = Connection::open(self.root.join(format!("{:016x}.sqlite3", hash.finish())))?;
        let version: u32 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version != 3 {
            db.execute_batch("DROP TABLE IF EXISTS records; DROP TABLE IF EXISTS files; DROP TABLE IF EXISTS contexts; DROP TABLE IF EXISTS transactions; DROP TABLE IF EXISTS observations; PRAGMA user_version=3;")?;
        }
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS metadata (path TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS files (name TEXT PRIMARY KEY, size INTEGER, modified TEXT, scanned INTEGER, damaged INTEGER);
             CREATE TABLE IF NOT EXISTS records (
               id INTEGER PRIMARY KEY, file TEXT NOT NULL, position INTEGER NOT NULL, length INTEGER NOT NULL,
               source TEXT, protocol TEXT, direction TEXT, txn TEXT, search TEXT, stamp INTEGER, seq INTEGER);
             CREATE INDEX IF NOT EXISTS records_time ON records(stamp,id);
             CREATE INDEX IF NOT EXISTS records_direction ON records(direction, id);
             CREATE INDEX IF NOT EXISTS records_transaction ON records(source, protocol, txn, id);"
        )?;
        crate::recording_analysis::schema(&db)?;
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
            tx.execute_batch("DELETE FROM records; DELETE FROM files; DELETE FROM contexts; DELETE FROM transactions; DELETE FROM observations;")?;
        }
        let metadata: serde_json::Value =
            serde_json::from_slice(&fs::read(folder.join("session.json"))?)?;
        tx.execute(
            "INSERT OR IGNORE INTO contexts VALUES (0,?)",
            [metadata
                .get("context")
                .cloned()
                .unwrap_or(serde_json::json!({}))
                .to_string()],
        )?;
        tx.execute_batch("CREATE TEMP TABLE dirty_transactions(source TEXT, protocol TEXT, txn TEXT, PRIMARY KEY(source,protocol,txn));")?;
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
                        tx.execute("INSERT INTO records(file,position,length,source,protocol,direction,txn,search,stamp,seq) VALUES (?,?,?,?,?,?,?,?,?,?)",
                            params![name, position, line.len() as u64, r.source, r.protocol, r.direction, r.transaction.to_string(), search, r.timestamp_us, r.sequence])?;
                        crate::recording_analysis::observation(&tx, &r)?;
                        if r.protocol == "recording-context"
                            && serde_json::from_str::<serde_json::Value>(&r.detail).is_ok()
                        {
                            tx.execute(
                                "INSERT OR REPLACE INTO contexts VALUES (?,?)",
                                params![r.sequence, r.detail],
                            )?;
                        }
                        if r.transaction != 0
                            && ["modbus-rtu", "modbus-ascii"].contains(&r.protocol.as_str())
                        {
                            tx.execute(
                                "INSERT OR IGNORE INTO dirty_transactions VALUES (?,?,?)",
                                params![r.source, r.protocol, r.transaction.to_string()],
                            )?;
                        }
                    }
                    Err(_) => damaged += 1,
                }
                position += line.len() as u64;
            }
            tx.execute("INSERT INTO files VALUES (?,?,?,?,?) ON CONFLICT(name) DO UPDATE SET size=excluded.size, modified=excluded.modified, scanned=excluded.scanned, damaged=files.damaged+excluded.damaged",
                params![name, size, modified, position, damaged])?;
        }
        let mut dirty = tx.prepare("SELECT source,protocol,txn FROM dirty_transactions")?;
        for entry in dirty.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })? {
            let (source, protocol, txn) = entry?;
            let mut stmt = tx.prepare("SELECT file,position,length FROM records WHERE source=? AND protocol=? AND txn=? ORDER BY id LIMIT 4097")?;
            let mut records = Vec::new();
            let mut bytes = 0_u64;
            for row in stmt.query_map(params![source, protocol, txn], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })? {
                let location: (String, u64, u64) = row?;
                bytes += location.2;
                if bytes > 2 * 1024 * 1024 || records.len() >= 4096 {
                    records.clear();
                    break;
                }
                records.push(read_record(&folder, location)?);
            }
            if !records.is_empty() {
                crate::recording_analysis::update(&tx, &records)?;
            } else {
                tx.execute(
                    "DELETE FROM transactions WHERE source=? AND protocol=? AND txn=?",
                    params![source, protocol, txn],
                )?;
            }
        }
        drop(dirty);
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
            let (condition, mut values) = filter.condition().map_err(std::io::Error::other)?;
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
    pub fn overview(&self, path: &str) -> std::result::Result<Overview, String> {
        let run = || -> Result<Overview> {
            let _guard = self.lock.lock().map_err(|_| "录制索引锁损坏")?;
            let (db, _, warning) = self.sync(path)?;
            let (first_us, last_us, count) = db.query_row(
                "SELECT MIN(NULLIF(stamp,0)),MAX(NULLIF(stamp,0)),COUNT(*) FROM records",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            let channels = db.prepare("SELECT source,protocol,slave,MIN(address),MAX(address+count-1) FROM transactions WHERE function=3 AND outcome='success' GROUP BY source,protocol,slave UNION ALL SELECT source,protocol,0,MIN(address),MAX(address) FROM observations GROUP BY source,protocol ORDER BY source,protocol,slave")?
                .query_map([],|r| Ok(Channel { source:r.get(0)?,protocol:r.get(1)?,slave:r.get(2)?,first_address:r.get(3)?,last_address:r.get(4)? }))?.collect::<std::result::Result<Vec<_>,_>>()?;
            Ok(Overview {
                first_us,
                last_us,
                count,
                channels,
                warning,
            })
        };
        run().map_err(|e| e.to_string())
    }
    pub fn context(
        &self,
        path: &str,
        sequence: u64,
    ) -> std::result::Result<serde_json::Value, String> {
        let run = || -> Result<serde_json::Value> {
            let _guard = self.lock.lock().map_err(|_| "录制索引锁损坏")?;
            let (db, _, _) = self.sync(path)?;
            let json: String = db.query_row(
                "SELECT json FROM contexts WHERE seq<=? ORDER BY seq DESC LIMIT 1",
                [sequence],
                |r| r.get(0),
            )?;
            Ok(serde_json::from_str(&json)?)
        };
        run().map_err(|e| e.to_string())
    }
    pub fn history(
        &self,
        path: &str,
        filter: &RecordFilter,
        offset: u64,
    ) -> std::result::Result<History, String> {
        let run = || -> Result<History> {
            filter.condition().map_err(std::io::Error::other)?;
            let address = filter.address.ok_or("请选择历史寄存器地址")?;
            let slave = filter.slave.ok_or("请选择历史通道")?;
            if filter.source.is_empty() || filter.protocol.is_empty() {
                return Err("请选择来源与协议".into());
            }
            let _guard = self.lock.lock().map_err(|_| "录制索引锁损坏")?;
            let (db, _, warning) = self.sync(path)?;
            if filter.protocol == "status-sample" {
                let condition = " WHERE source=? AND address=? AND stamp>=? AND stamp<=?";
                let values = params![
                    filter.source,
                    address,
                    filter.from_us.unwrap_or(0),
                    filter.to_us.unwrap_or(i64::MAX as u64)
                ];
                let total: u64 = db.query_row(
                    &format!("SELECT COUNT(*) FROM observations{condition}"),
                    values,
                    |r| r.get(0),
                )?;
                let offset = if offset >= total {
                    total.saturating_sub(2000)
                } else {
                    offset
                };
                let points = db.prepare(&format!("SELECT seq,stamp,source,protocol,raw,value,name,unit FROM observations{condition} ORDER BY stamp,seq LIMIT 2000 OFFSET {offset}"))?
                    .query_map(values,|r| Ok(crate::recording_analysis::Point { sequence:r.get(0)?,timestamp_us:r.get(1)?,source:r.get(2)?,protocol:r.get(3)?,transaction:0,raw:r.get(4)?,value:r.get(5)?,name:r.get(6)?,unit:r.get(7)? }))?.collect::<std::result::Result<Vec<_>,_>>()?;
                return Ok(History {
                    points,
                    total,
                    offset,
                    warning,
                });
            }
            let condition = " WHERE source=? AND protocol=? AND slave=? AND function=3 AND outcome='success' AND address<=? AND address+count>? AND stamp>=? AND stamp<=?";
            let values = params![
                filter.source,
                filter.protocol,
                slave,
                address,
                address,
                filter.from_us.unwrap_or(0),
                filter.to_us.unwrap_or(i64::MAX as u64)
            ];
            let total: u64 = db.query_row(
                &format!("SELECT COUNT(*) FROM transactions{condition}"),
                values,
                |r| r.get(0),
            )?;
            let offset = if offset >= total {
                total.saturating_sub(2000)
            } else {
                offset
            };
            let mut stmt = db.prepare(&format!("SELECT seq,stamp,json FROM transactions{condition} ORDER BY stamp,seq LIMIT 2000 OFFSET {offset}"))?;
            let mut points = Vec::new();
            for row in stmt.query_map(values, |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))? {
                let (seq, stamp, json) = row?;
                if let Some(point) =
                    crate::recording_analysis::point(&db, seq, stamp, json, address)?
                {
                    points.push(point);
                }
            }
            Ok(History {
                points,
                total,
                offset,
                warning,
            })
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
#[tauri::command]
pub async fn recording_overview(
    path: String,
    state: State<'_, RecordingIndex>,
) -> std::result::Result<Overview, String> {
    let index = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || index.overview(&path))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn recording_context(
    path: String,
    sequence: u64,
    state: State<'_, RecordingIndex>,
) -> std::result::Result<serde_json::Value, String> {
    let index = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || index.context(&path, sequence))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn recording_history(
    path: String,
    filter: RecordFilter,
    offset: u64,
    state: State<'_, RecordingIndex>,
) -> std::result::Result<History, String> {
    let index = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || index.history(&path, &filter, offset))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn structured_filters_history_context_and_simulator_samples() {
        let root =
            std::env::temp_dir().join(format!("servo-analysis-{}", recording::timestamp_us()));
        let recorder = recording::Recorder::new(root.clone());
        let mut profile: serde_json::Value =
            serde_json::from_str(include_str!("../../examples/servo-profile.example.json"))
                .unwrap();
        profile["statuses"] = serde_json::json!([{"id":"speed","name":"速度","address":16,"rawType":"i16","decimals":1,"unit":"rpm"}]);
        recorder.set_context("profile", profile.clone());
        recorder.start(16).unwrap();
        let rtu = |p: &[u8]| {
            let mut b = p.to_vec();
            b.extend_from_slice(&crate::modbus::modbus_crc(p).to_le_bytes());
            b
        };
        for n in 1..=3 {
            if n == 2 {
                profile["statuses"][0]["decimals"] = serde_json::json!(0);
                recorder.set_context("profile", profile.clone());
            }
            recorder.emit_at(
                "test-port",
                "modbus-rtu",
                "tx",
                n,
                &rtu(&[1, 3, 0, 16, 0, 1]),
                "",
                n * 100,
            );
            let mut rx = rtu(&[1, 3, 2, 255, 255]);
            if n == 3 {
                rx[0] ^= 1;
            }
            recorder.emit_at("test-port", "modbus-rtu", "rx", n, &rx, "", n * 100 + 10);
            recorder.emit_at(
                "test-port",
                "modbus-rtu",
                "event",
                n,
                &[],
                if n == 3 {
                    "FC03 request=[] result=Err(Crc)"
                } else {
                    "FC03 request=[] result=Ok(success)"
                },
                n * 100 + 20,
            );
        }
        recorder.emit(
            "simulator",
            "status-sample",
            "event",
            0,
            &[],
            r#"[{"id":"s","name":"模拟","address":32,"raw":5,"value":0.5,"unit":"A"}]"#,
        );
        let stopped = recorder.stop().unwrap();
        let index = RecordingIndex::new(root.join("cache"));
        let mut filter = RecordFilter {
            slave: Some(1),
            function: Some(3),
            address: Some(16),
            outcome: "success".into(),
            ..Default::default()
        };
        assert_eq!(
            index.page(&stopped.path, 0, &filter, false).unwrap().total,
            6
        );
        filter.outcome = "failure".into();
        assert_eq!(
            index.page(&stopped.path, 0, &filter, false).unwrap().total,
            3
        );
        filter.outcome = "success".into();
        filter.from_us = Some(200);
        filter.to_us = Some(220);
        assert_eq!(
            index.page(&stopped.path, 0, &filter, false).unwrap().total,
            3
        );
        filter.from_us = None;
        filter.to_us = None;
        filter.source = "test-port".into();
        filter.protocol = "modbus-rtu".into();
        let history = index.history(&stopped.path, &filter, 0).unwrap();
        assert_eq!(history.total, 2);
        assert_eq!(history.points[0].value, -0.1);
        assert_eq!(history.points[1].value, -1.0);
        assert_eq!(history.points[0].transaction, 1);
        assert_eq!(
            index
                .context(&stopped.path, history.points[0].sequence)
                .unwrap()["profile"]["statuses"][0]["decimals"],
            1
        );
        assert_eq!(index.overview(&stopped.path).unwrap().channels.len(), 2);
        filter.source = "simulator".into();
        filter.protocol = "status-sample".into();
        filter.slave = Some(0);
        filter.address = Some(32);
        let history = index.history(&stopped.path, &filter, 0).unwrap();
        assert_eq!(history.points[0].value, 0.5);
        fs::remove_dir_all(root).unwrap();
    }
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
                    ..Default::default()
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
