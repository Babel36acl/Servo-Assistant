use crate::runtime::{capture_parameter_snapshot, AppState, ParameterSnapshot};
use serde::Serialize;
use std::io::Write;
use std::path::Path;
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotExport {
    path: String,
    parameter_count: usize,
}

#[tauri::command]
pub async fn export_parameter_snapshot(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Option<SnapshotExport>, String> {
    let mut dialog = app
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("导出参数快照 — 选择保存位置")
        .add_filter("参数快照 (*.servo-snapshot.json)", &["json"])
        .set_file_name(format!(
            "servo-parameters_{}.servo-snapshot.json",
            crate::audit::now_ms()
        ));
    if let Ok(directory) = app.path().download_dir() {
        dialog = dialog.set_directory(directory);
    }
    let Some(selected) = dialog.blocking_save_file() else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|e| format!("保存路径无效：{e}"))?;
    let snapshot = capture_parameter_snapshot(Some("手动导出".into()), state).await?;
    save_snapshot(&path, &snapshot)
        .map_err(|e| format!("快照已存入审计库，但文件导出失败：{e}"))?;
    Ok(Some(SnapshotExport {
        path: path.to_string_lossy().into_owned(),
        parameter_count: snapshot.values.len(),
    }))
}

fn save_snapshot(path: &Path, snapshot: &ParameterSnapshot) -> Result<(), String> {
    let contents = serde_json::to_vec_pretty(snapshot).map_err(|e| e.to_string())?;
    let directory = path
        .parent()
        .ok_or_else(|| "保存路径没有父目录".to_string())?;
    // 原生保存对话框负责覆盖确认；先写同目录临时文件，成功后才替换目标备份。
    let mut file = tempfile::NamedTempFile::new_in(directory)
        .map_err(|e| format!("{}：{e}", path.display()))?;
    file.write_all(&contents)
        .and_then(|_| file.as_file().sync_all())
        .map_err(|e| format!("{}：{e}", path.display()))?;
    file.persist(path)
        .map_err(|e| format!("{}：{e}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_round_trips_to_selected_path_and_handles_replacement_and_failure() {
        let directory = tempfile::tempdir().unwrap();
        let snapshot = ParameterSnapshot {
            schema_version: "servo-parameter-snapshot/1.0".into(),
            created_at_ms: 123,
            label: "导出测试".into(),
            device_id: "../HD3:test".into(),
            device_name: "伺服".into(),
            profile_version: "1.0.0".into(),
            values: vec![crate::runtime::SnapshotValue {
                parameter_id: "P137".into(),
                raw: 65535,
                value: -1.0,
            }],
        };
        let path = directory.path().join("自选备份.servo-snapshot.json");
        std::fs::write(&path, "old backup").unwrap();
        save_snapshot(&path, &snapshot).unwrap();
        let contents = std::fs::read(&path).unwrap();
        let restored: ParameterSnapshot = serde_json::from_slice(&contents).unwrap();
        assert_eq!(restored.schema_version, snapshot.schema_version);
        assert_eq!(restored.values[0].raw, 65535);
        assert_eq!(restored.values[0].value, -1.0);
        assert!(save_snapshot(&directory.path().join("missing/file.json"), &snapshot).is_err());
        assert!(save_snapshot(directory.path(), &snapshot).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), contents);
    }
}
