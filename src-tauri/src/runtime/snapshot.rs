//! 参数快照：导出当前值，并与给定的快照做差异比较。
use super::params::read_parameter_definitions;
use super::types::{ParameterSnapshot, SnapshotDiff, SnapshotValue};
use super::{with_runtime, AppState, RuntimeError};
use crate::audit::now_ms;
use crate::profile::{decode_value, encode_value, Access};
use std::collections::{HashMap, HashSet};
use tauri::State;

#[tauri::command]
pub async fn capture_parameter_snapshot(
    label: Option<String>,
    state: State<'_, AppState>,
) -> Result<ParameterSnapshot, String> {
    with_runtime(state, move |runtime| {
        let profile = runtime.profile.clone().ok_or(RuntimeError::NoProfile)?;
        let session = runtime.session.as_mut().ok_or(RuntimeError::NotConnected)?;
        let values = read_parameter_definitions(session, profile.parameters.clone())?
            .into_iter()
            .map(|value| SnapshotValue {
                parameter_id: value.parameter_id,
                raw: value.raw,
                value: value.value,
            })
            .collect();
        let label = label
            .unwrap_or_else(|| "手动导出".into())
            .trim()
            .chars()
            .take(120)
            .collect::<String>();
        let snapshot = ParameterSnapshot {
            schema_version: "servo-parameter-snapshot/1.0".into(),
            created_at_ms: now_ms(),
            label,
            device_id: profile.device.id.clone(),
            device_name: profile.device.name.clone(),
            profile_version: profile.device.profile_version.clone(),
            values,
        };
        let snapshot_json = serde_json::to_string(&snapshot)
            .map_err(|error| RuntimeError::Snapshot(error.to_string()))?;
        runtime.audit_store.save_snapshot(
            snapshot.created_at_ms,
            &snapshot.device_id,
            &snapshot.profile_version,
            &snapshot.label,
            &snapshot_json,
        )?;
        runtime.push_audit(
            "snapshot.export",
            "success",
            &format!("{}，{} 个参数", snapshot.label, snapshot.values.len()),
        )?;
        Ok(snapshot)
    })
    .await
}

#[tauri::command]
pub async fn compare_parameter_snapshot(
    snapshot_json: String,
    state: State<'_, AppState>,
) -> Result<Vec<SnapshotDiff>, String> {
    with_runtime(state, move |runtime| {
        let snapshot: ParameterSnapshot = serde_json::from_str(&snapshot_json)
            .map_err(|error| RuntimeError::Snapshot(error.to_string()))?;
        if snapshot.schema_version != "servo-parameter-snapshot/1.0" {
            return Err(RuntimeError::Snapshot(format!(
                "不支持 schemaVersion={}",
                snapshot.schema_version
            )));
        }
        let profile = runtime.profile.clone().ok_or(RuntimeError::NoProfile)?;
        if snapshot.device_id != profile.device.id {
            return Err(RuntimeError::Snapshot(format!(
                "设备不匹配：快照为 {}，当前为 {}",
                snapshot.device_id, profile.device.id
            )));
        }
        if snapshot.profile_version != profile.device.profile_version {
            return Err(RuntimeError::Snapshot(format!(
                "Profile 版本不匹配：快照为 {}，当前为 {}",
                snapshot.profile_version, profile.device.profile_version
            )));
        }
        let mut seen = HashSet::new();
        let mut definitions = Vec::with_capacity(snapshot.values.len());
        for value in &snapshot.values {
            if !seen.insert(value.parameter_id.as_str()) {
                return Err(RuntimeError::Snapshot(format!(
                    "参数重复：{}",
                    value.parameter_id
                )));
            }
            definitions.push(
                profile
                    .parameter(&value.parameter_id)
                    .cloned()
                    .ok_or_else(|| RuntimeError::UnknownParameter(value.parameter_id.clone()))?,
            );
        }
        let session = runtime.session.as_mut().ok_or(RuntimeError::NotConnected)?;
        let current = read_parameter_definitions(session, definitions)?
            .into_iter()
            .map(|value| (value.parameter_id.clone(), value))
            .collect::<HashMap<_, _>>();
        let targets = snapshot
            .values
            .into_iter()
            .map(|value| (value.parameter_id.clone(), value))
            .collect::<HashMap<_, _>>();
        let mut diffs = Vec::with_capacity(targets.len());
        for definition in &profile.parameters {
            let Some(target) = targets.get(&definition.parameter_id) else {
                continue;
            };
            let current = current
                .get(&definition.parameter_id)
                .ok_or_else(|| RuntimeError::UnknownParameter(definition.parameter_id.clone()))?;
            let encoded_target = encode_value(definition, target.value).map_err(|error| {
                RuntimeError::Snapshot(format!("{}：{}", definition.parameter_id, error))
            })?;
            if encoded_target != target.raw {
                return Err(RuntimeError::Snapshot(format!(
                    "{} 的 raw={} 与 value={} 不一致",
                    definition.parameter_id, target.raw, target.value
                )));
            }
            diffs.push(SnapshotDiff {
                parameter_id: definition.parameter_id.clone(),
                name: definition.name.clone(),
                current_raw: current.raw,
                current_value: current.value,
                target_raw: target.raw,
                target_value: decode_value(definition.raw_type, definition.decimals, target.raw),
                changed: current.raw != target.raw,
                writable: definition.access == Access::Rw,
                risk: definition.risk,
            });
        }
        let changed = diffs.iter().filter(|diff| diff.changed).count();
        runtime.push_audit(
            "snapshot.compare",
            "success",
            &format!("{}，发现 {changed} 项差异", snapshot.label),
        )?;
        Ok(diffs)
    })
    .await
}
