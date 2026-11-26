//! 参数读写：单写回读、批量写前校验、状态读取与分组读取辅助。
use super::types::{
    BatchWriteRequest, BatchWriteResult, ConnectionMode, ParameterValue, StatusValue, WriteRequest,
    WriteResult,
};
use super::{with_runtime, AppState, RuntimeError, Session};
use crate::profile::{
    decode_value, encode_value, Access, ParameterDefinition, RiskLevel, StatusDefinition,
};
use std::collections::{HashMap, HashSet};
use tauri::State;

#[tauri::command]
pub async fn read_parameters(
    parameter_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ParameterValue>, String> {
    with_runtime(state, move |runtime| {
        let profile = runtime.profile.clone().ok_or(RuntimeError::NoProfile)?;
        let definitions = if parameter_ids.is_empty() {
            profile.parameters.clone()
        } else {
            parameter_ids
                .iter()
                .map(|parameter_id| {
                    profile
                        .parameter(parameter_id)
                        .cloned()
                        .ok_or_else(|| RuntimeError::UnknownParameter(parameter_id.clone()))
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        let session = runtime.session.as_mut().ok_or(RuntimeError::NotConnected)?;
        read_parameter_definitions(session, definitions)
    })
    .await
}

#[tauri::command]
pub async fn read_statuses(state: State<'_, AppState>) -> Result<Vec<StatusValue>, String> {
    with_runtime(state, |runtime| {
        let statuses = runtime
            .profile
            .as_ref()
            .ok_or(RuntimeError::NoProfile)?
            .statuses
            .clone();
        let session = runtime.session.as_mut().ok_or(RuntimeError::NotConnected)?;
        let simulator = session.mode() == ConnectionMode::Simulator;
        let values = read_status_definitions(session, statuses)?;
        if simulator {
            runtime.recorder.emit(
                "simulator",
                "status-sample",
                "event",
                0,
                &[],
                &serde_json::json!(values).to_string(),
            );
        }
        Ok(values)
    })
    .await
}

#[tauri::command]
pub async fn write_parameter(
    request: WriteRequest,
    state: State<'_, AppState>,
) -> Result<WriteResult, String> {
    with_runtime(state, move |runtime| {
        if !request.confirmed {
            return Err(RuntimeError::Confirmation("必须明确确认本次写入".into()));
        }
        let profile = runtime.profile.clone().ok_or(RuntimeError::NoProfile)?;
        let definition = profile
            .parameter(&request.parameter_id)
            .cloned()
            .ok_or_else(|| RuntimeError::UnknownParameter(request.parameter_id.clone()))?;
        if definition.access != Access::Rw {
            return Err(RuntimeError::ReadOnly(definition.parameter_id));
        }
        if matches!(definition.risk, RiskLevel::High | RiskLevel::Critical) {
            let expected_phrase = format!("写入 {}", definition.parameter_id);
            if request.confirmation_phrase.as_deref() != Some(expected_phrase.as_str()) {
                return Err(RuntimeError::Confirmation(format!(
                    "高风险参数必须输入“{expected_phrase}”"
                )));
            }
        }
        let new_raw = encode_value(&definition, request.value)?;
        let previous_raw = runtime
            .session
            .as_mut()
            .ok_or(RuntimeError::NotConnected)?
            .read_registers(definition.address, 1)?[0];
        if let Some(expected) = request.expected_raw {
            if expected != previous_raw {
                runtime.push_audit(
                    "parameter.write",
                    "failure",
                    &format!(
                        "{} 写前值变化：期望 {}，实际 {}",
                        definition.parameter_id, expected, previous_raw
                    ),
                )?;
                return Err(RuntimeError::StaleValue {
                    parameter_id: definition.parameter_id.clone(),
                    expected,
                    actual: previous_raw,
                });
            }
        }
        runtime.push_audit(
            "parameter.write",
            "attempt",
            &format!(
                "{} raw {} -> {}",
                definition.parameter_id, previous_raw, new_raw
            ),
        )?;
        let write_result = (|| {
            let session = runtime.session.as_mut().ok_or(RuntimeError::NotConnected)?;
            session.write_register(definition.address, new_raw)?;
            let read_back_raw = session.read_registers(definition.address, 1)?[0];
            if read_back_raw != new_raw {
                return Err(RuntimeError::ReadBackMismatch {
                    parameter_id: definition.parameter_id.clone(),
                    written: new_raw,
                    read_back: read_back_raw,
                });
            }
            Ok(read_back_raw)
        })();
        let read_back_raw = match write_result {
            Ok(value) => value,
            Err(error) => {
                runtime.push_audit(
                    "parameter.write",
                    "failure",
                    &format!("{}：{}", definition.parameter_id, error),
                )?;
                return Err(error);
            }
        };
        let result = WriteResult {
            parameter_id: definition.parameter_id.clone(),
            previous_raw,
            written_raw: new_raw,
            read_back_raw,
            value: decode_value(definition.raw_type, definition.decimals, read_back_raw),
        };
        runtime.push_audit(
            "parameter.write",
            "success",
            &format!(
                "{} raw {} -> {}，回读一致",
                definition.parameter_id, previous_raw, read_back_raw
            ),
        )?;
        Ok(result)
    })
    .await
}

#[tauri::command]
pub async fn batch_write_parameters(
    request: BatchWriteRequest,
    state: State<'_, AppState>,
) -> Result<BatchWriteResult, String> {
    with_runtime(state, move |runtime| {
        if request.items.is_empty() || request.items.len() > 500 {
            return Err(RuntimeError::Confirmation(
                "批量写入数量必须为 1..500".into(),
            ));
        }
        let expected_phrase = format!("批量写入 {} 项", request.items.len());
        if request.confirmation_phrase != expected_phrase {
            return Err(RuntimeError::Confirmation(format!(
                "必须输入“{expected_phrase}”"
            )));
        }
        let profile = runtime.profile.clone().ok_or(RuntimeError::NoProfile)?;
        let mut seen = HashSet::new();
        let mut planned = Vec::with_capacity(request.items.len());
        for item in request.items {
            if !seen.insert(item.parameter_id.clone()) {
                return Err(RuntimeError::Confirmation(format!(
                    "批量列表参数重复：{}",
                    item.parameter_id
                )));
            }
            let definition = profile
                .parameter(&item.parameter_id)
                .cloned()
                .ok_or_else(|| RuntimeError::UnknownParameter(item.parameter_id.clone()))?;
            if definition.access != Access::Rw {
                return Err(RuntimeError::ReadOnly(definition.parameter_id));
            }
            let new_raw = encode_value(&definition, item.value)?;
            planned.push((definition, new_raw, item.expected_raw));
        }

        let preflight = {
            let session = runtime.session.as_mut().ok_or(RuntimeError::NotConnected)?;
            preflight_planned_writes(session, &planned)
        };
        if let Err(error) = preflight {
            runtime.push_audit(
                "parameter.batch",
                "failure",
                &format!("写前校验失败，整批未开始：{error}"),
            )?;
            return Err(error);
        }
        runtime.push_audit(
            "parameter.batch",
            "attempt",
            &format!("已完成写前校验，准备写入 {} 项", planned.len()),
        )?;

        let mut completed = Vec::with_capacity(planned.len());
        for (definition, new_raw, previous_raw) in planned {
            let result = write_planned_parameter(
                runtime.session.as_mut().ok_or(RuntimeError::NotConnected)?,
                &definition,
                new_raw,
                previous_raw,
            );
            match result {
                Ok(result) => {
                    runtime.push_audit(
                        "parameter.write",
                        "success",
                        &format!(
                            "{} raw {} -> {}，批量写入回读一致",
                            result.parameter_id, result.previous_raw, result.read_back_raw
                        ),
                    )?;
                    completed.push(result);
                }
                Err(error) => {
                    runtime.push_audit(
                        "parameter.batch",
                        "failure",
                        &format!(
                            "已完成 {} 项，{} 失败：{}",
                            completed.len(),
                            definition.parameter_id,
                            error
                        ),
                    )?;
                    return Ok(BatchWriteResult {
                        completed,
                        failed_parameter_id: Some(definition.parameter_id),
                        error: Some(error.to_string()),
                    });
                }
            }
        }
        runtime.push_audit(
            "parameter.batch",
            "success",
            &format!("{} 项全部写入并回读一致", completed.len()),
        )?;
        Ok(BatchWriteResult {
            completed,
            failed_parameter_id: None,
            error: None,
        })
    })
    .await
}

type PlannedWrite = (ParameterDefinition, u16, u16);

pub(super) fn read_parameter_definitions(
    session: &mut Session,
    mut definitions: Vec<ParameterDefinition>,
) -> Result<Vec<ParameterValue>, RuntimeError> {
    definitions.sort_by_key(|definition| definition.address);
    let mut values = Vec::with_capacity(definitions.len());
    let mut cursor = 0;
    while cursor < definitions.len() {
        let start = definitions[cursor].address;
        let mut end = cursor + 1;
        while end < definitions.len()
            && definitions[end].address == definitions[end - 1].address + 1
            && definitions[end].address - start < 100
        {
            end += 1;
        }
        let count = definitions[end - 1].address - start + 1;
        let registers = session.read_registers(start, count)?;
        for definition in &definitions[cursor..end] {
            let raw = registers[(definition.address - start) as usize];
            values.push(ParameterValue {
                parameter_id: definition.parameter_id.clone(),
                raw,
                value: decode_value(definition.raw_type, definition.decimals, raw),
            });
        }
        cursor = end;
    }
    Ok(values)
}

pub(super) fn read_status_definitions(
    session: &mut Session,
    mut definitions: Vec<StatusDefinition>,
) -> Result<Vec<StatusValue>, RuntimeError> {
    definitions.sort_by_key(|definition| definition.address);
    let mut values = Vec::with_capacity(definitions.len());
    let mut cursor = 0;
    while cursor < definitions.len() {
        let start = definitions[cursor].address;
        let mut end = cursor + 1;
        while end < definitions.len()
            && definitions[end].address == definitions[end - 1].address + 1
            && definitions[end].address - start < 100
        {
            end += 1;
        }
        let count = definitions[end - 1].address - start + 1;
        let registers = session.read_registers(start, count)?;
        for definition in &definitions[cursor..end] {
            let raw = registers[(definition.address - start) as usize];
            values.push(StatusValue {
                id: definition.id.clone(),
                name: definition.name.clone(),
                address: definition.address,
                raw,
                value: decode_value(definition.raw_type, definition.decimals, raw),
                unit: definition.unit.clone(),
            });
        }
        cursor = end;
    }
    Ok(values)
}

pub(super) fn preflight_planned_writes(
    session: &mut Session,
    planned: &[PlannedWrite],
) -> Result<(), RuntimeError> {
    let current_values = read_parameter_definitions(
        session,
        planned
            .iter()
            .map(|(definition, _, _)| definition.clone())
            .collect(),
    )?
    .into_iter()
    .map(|value| (value.parameter_id, value.raw))
    .collect::<HashMap<_, _>>();
    for (definition, _, expected_raw) in planned {
        let actual = *current_values
            .get(&definition.parameter_id)
            .ok_or_else(|| RuntimeError::UnknownParameter(definition.parameter_id.clone()))?;
        if actual != *expected_raw {
            return Err(RuntimeError::StaleValue {
                parameter_id: definition.parameter_id.clone(),
                expected: *expected_raw,
                actual,
            });
        }
    }
    Ok(())
}

pub(super) fn write_planned_parameter(
    session: &mut Session,
    definition: &ParameterDefinition,
    new_raw: u16,
    previous_raw: u16,
) -> Result<WriteResult, RuntimeError> {
    session.write_register(definition.address, new_raw)?;
    let read_back = session.read_registers(definition.address, 1)?[0];
    if read_back != new_raw {
        return Err(RuntimeError::ReadBackMismatch {
            parameter_id: definition.parameter_id.clone(),
            written: new_raw,
            read_back,
        });
    }
    Ok(WriteResult {
        parameter_id: definition.parameter_id.clone(),
        previous_raw,
        written_raw: new_raw,
        read_back_raw: read_back,
        value: decode_value(definition.raw_type, definition.decimals, read_back),
    })
}
