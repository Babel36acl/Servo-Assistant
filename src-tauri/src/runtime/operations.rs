//! 应用/持久化操作：写命令寄存器并轮询状态寄存器，同时记录审计。
use super::types::OperationResult;
use super::{with_runtime, AppState, Runtime, RuntimeError};
use crate::profile::{OperationDefinition, OperationSet};
use std::time::Duration;
use tauri::State;

#[tauri::command]
pub async fn apply_parameters(
    confirmation_phrase: String,
    state: State<'_, AppState>,
) -> Result<OperationResult, String> {
    if confirmation_phrase != "应用参数" {
        return Err("必须输入“应用参数”后才能执行".into());
    }
    with_runtime(state, |runtime| {
        let operations = runtime
            .profile
            .as_ref()
            .ok_or(RuntimeError::NoProfile)?
            .operations
            .clone()
            .ok_or_else(|| RuntimeError::UnsupportedOperation("应用参数".into()))?;
        let operation = operations
            .apply
            .clone()
            .ok_or_else(|| RuntimeError::UnsupportedOperation("应用参数".into()))?;
        execute_audited_operation(
            runtime,
            "parameters.apply",
            "应用参数",
            &operations,
            &operation,
        )
    })
    .await
}

#[tauri::command]
pub async fn persist_parameters(
    confirmation_phrase: String,
    state: State<'_, AppState>,
) -> Result<OperationResult, String> {
    if confirmation_phrase != "持久化参数" {
        return Err("必须输入“持久化参数”后才能执行".into());
    }
    with_runtime(state, |runtime| {
        let operations = runtime
            .profile
            .as_ref()
            .ok_or(RuntimeError::NoProfile)?
            .operations
            .clone()
            .ok_or_else(|| RuntimeError::UnsupportedOperation("持久化".into()))?;
        let operation = operations
            .persist
            .clone()
            .ok_or_else(|| RuntimeError::UnsupportedOperation("持久化".into()))?;
        execute_audited_operation(
            runtime,
            "parameters.persist",
            "持久化参数",
            &operations,
            &operation,
        )
    })
    .await
}

fn execute_operation(
    runtime: &mut Runtime,
    name: &str,
    operations: &OperationSet,
    operation: &OperationDefinition,
) -> Result<OperationResult, RuntimeError> {
    let session = runtime.session.as_mut().ok_or(RuntimeError::NotConnected)?;
    session.write_register(operations.command_register, operation.command)?;
    let started = std::time::Instant::now();
    while started.elapsed() <= Duration::from_millis(operation.timeout_ms) {
        let observed = session.read_registers(operations.status_register, 1)?[0];
        if observed == operation.success_status {
            return Ok(OperationResult {
                operation: name.into(),
                command: operation.command,
                observed_status: observed,
            });
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(RuntimeError::OperationTimeout(name.into()))
}

fn execute_audited_operation(
    runtime: &mut Runtime,
    action: &str,
    name: &str,
    operations: &OperationSet,
    operation: &OperationDefinition,
) -> Result<OperationResult, RuntimeError> {
    runtime.push_audit(
        action,
        "attempt",
        &format!("写入命令 0x{:04X}", operation.command),
    )?;
    match execute_operation(runtime, name, operations, operation) {
        Ok(result) => {
            runtime.push_audit(
                action,
                "success",
                &format!("设备返回 0x{:04X}", result.observed_status),
            )?;
            Ok(result)
        }
        Err(error) => {
            runtime.push_audit(action, "failure", &error.to_string())?;
            Err(error)
        }
    }
}
