//! Derived transaction metadata. The raw records and captured context remain authoritative.
use crate::{
    modbus_decode,
    profile::{self, ServoProfile},
    recording::Record,
};
use rusqlite::{params, Connection};
use serde::Serialize;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub fn schema(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS contexts(seq INTEGER PRIMARY KEY, json TEXT);
        CREATE TABLE IF NOT EXISTS observations(seq INTEGER, address INTEGER, source TEXT, protocol TEXT, stamp INTEGER, raw INTEGER, value REAL, name TEXT, unit TEXT, PRIMARY KEY(seq,address));
        CREATE INDEX IF NOT EXISTS observations_time ON observations(source,address,stamp);
        CREATE TABLE IF NOT EXISTS transactions(source TEXT, protocol TEXT, txn TEXT, seq INTEGER, stamp INTEGER,
        slave INTEGER, function INTEGER, address INTEGER, count INTEGER, outcome TEXT, json TEXT,
        PRIMARY KEY(source,protocol,txn));
        CREATE INDEX IF NOT EXISTS transactions_time ON transactions(stamp);
        CREATE INDEX IF NOT EXISTS transactions_fields ON transactions(slave,function,address);")?;
    Ok(())
}
pub fn observation(db: &Connection, record: &Record) -> Result<()> {
    if record.protocol != "status-sample" {
        return Ok(());
    }
    if let Ok(values) = serde_json::from_str::<Vec<crate::runtime::StatusValue>>(&record.detail) {
        for v in values.into_iter().filter(|v| v.value.is_finite()) {
            db.execute(
                "INSERT OR REPLACE INTO observations VALUES (?,?,?,?,?,?,?,?,?)",
                params![
                    record.sequence,
                    v.address,
                    record.source,
                    record.protocol,
                    record.timestamp_us,
                    v.raw,
                    v.value,
                    v.name,
                    v.unit
                ],
            )?;
        }
    }
    Ok(())
}
pub fn update(db: &Connection, records: &[Record]) -> Result<()> {
    let first = records.first().ok_or("空事务")?;
    let decoded = modbus_decode::decode(records, None);
    let (slave, function, address, count, outcome, json) = match decoded {
        Ok(t) => {
            let outcome = if t.outcome == "请求与响应匹配" {
                "success"
            } else if t.outcome.starts_with("未记录到") {
                "incomplete"
            } else {
                "failure"
            };
            (
                t.request.slave,
                t.request.function,
                t.request.address,
                t.request.count,
                outcome,
                serde_json::to_string(&t)?,
            )
        }
        Err(_) => (None, None, None, None, "failure", "null".into()),
    };
    db.execute(
        "INSERT OR REPLACE INTO transactions VALUES (?,?,?,?,?,?,?,?,?,?,?)",
        params![
            first.source,
            first.protocol,
            first.transaction.to_string(),
            first.sequence,
            first.timestamp_us,
            slave,
            function,
            address,
            count,
            outcome,
            json
        ],
    )?;
    Ok(())
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub sequence: u64,
    pub timestamp_us: u64,
    pub source: String,
    pub protocol: String,
    pub transaction: u64,
    pub raw: u16,
    pub value: f64,
    pub name: String,
    pub unit: String,
}
pub fn point(
    db: &Connection,
    sequence: u64,
    timestamp_us: u64,
    json: String,
    address: u16,
) -> Result<Option<Point>> {
    let Ok(t) = serde_json::from_str::<modbus_decode::Transaction>(&json) else {
        return Ok(None);
    };
    if t.outcome != "请求与响应匹配" || t.request.function != Some(3) {
        return Ok(None);
    }
    let Some(start) = t.request.address else {
        return Ok(None);
    };
    let Some(raw) = address
        .checked_sub(start)
        .and_then(|i| t.response.values.get(i as usize))
        .copied()
    else {
        return Ok(None);
    };
    let context: String = db.query_row(
        "SELECT json FROM contexts WHERE seq<=? ORDER BY seq DESC LIMIT 1",
        [sequence],
        |r| r.get(0),
    )?;
    let context: serde_json::Value = serde_json::from_str(&context)?;
    let definition = serde_json::from_value::<ServoProfile>(context["profile"].clone())
        .ok()
        .and_then(|p| p.statuses.into_iter().find(|s| s.address == address));
    let (value, name, unit) = definition.map_or(
        (
            raw as f64,
            format!("寄存器 {address} · 原始值"),
            String::new(),
        ),
        |d| {
            (
                profile::decode_value(d.raw_type, d.decimals, raw),
                d.name,
                d.unit,
            )
        },
    );
    Ok(Some(Point {
        sequence,
        timestamp_us,
        source: t.source,
        protocol: t.protocol,
        transaction: t.transaction,
        raw,
        value,
        name,
        unit,
    }))
}
