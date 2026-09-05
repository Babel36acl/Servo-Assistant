//! Session annotations and file selection. Raw recordings are never rewritten.
use crate::recording::{self, Recorder};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};
use tauri::State;

pub fn save_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let temporary = path.with_extension("tmp");
    let mut file = File::create(&temporary).map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?)
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())?;
    drop(file);
    fs::rename(temporary, path).map_err(|e| e.to_string())
}

#[derive(Default, Serialize, Deserialize)]
pub struct Annotation {
    pub name: String,
    pub notes: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub path: String,
    pub annotation: Annotation,
    pub bytes: u64,
    pub metadata: serde_json::Value,
    pub warning: Option<String>,
}
pub fn info(path: &str) -> Result<SessionInfo, String> {
    let folder = Path::new(path);
    let files = recording::session_files(folder)?;
    let mut bytes = 0;
    for file in files {
        bytes += file.metadata().map_err(|e| e.to_string())?.len();
    }
    let annotation = if folder.join("annotation.json").exists() {
        serde_json::from_slice(
            &fs::read(folder.join("annotation.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?
    } else {
        Annotation::default()
    };
    Ok(SessionInfo {
        path: path.into(),
        annotation,
        bytes,
        metadata: serde_json::from_slice(
            &fs::read(folder.join("session.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?,
        warning: recording::session_warning(folder),
    })
}
#[tauri::command]
pub async fn recording_info(path: String) -> Result<SessionInfo, String> {
    tauri::async_runtime::spawn_blocking(move || info(&path))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn recording_annotate(path: String, annotation: Annotation) -> Result<(), String> {
    if annotation.name.chars().count() > 100 || annotation.notes.chars().count() > 4000 {
        return Err("名称最多 100 字，备注最多 4000 字".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        recording::session_files(Path::new(&path))?;
        save_json(&Path::new(&path).join("annotation.json"), &annotation)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn recording_mapping(
    config: serde_json::Value,
    state: State<'_, Recorder>,
) -> Result<(), String> {
    if config.to_string().len() > 1024 * 1024 {
        return Err("映射超过 1 MiB".into());
    }
    let parsed = serde_json::from_value(config.clone()).map_err(|e| e.to_string())?;
    crate::ethercat::decode_ethercat(vec![], parsed)?;
    state.set_context("mapping", config);
    Ok(())
}
#[derive(Serialize)]
pub struct Entry {
    pub path: String,
    pub name: String,
    pub directory: bool,
}
#[derive(Serialize)]
pub struct Directory {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<Entry>,
}
#[tauri::command]
pub async fn recording_browse(
    path: Option<String>,
    state: State<'_, Recorder>,
) -> Result<Directory, String> {
    let default = state
        .sessions()?
        .first()
        .and_then(|s| Path::new(s).parent())
        .map(Path::to_path_buf)
        .unwrap_or(std::env::current_dir().map_err(|e| e.to_string())?);
    tauri::async_runtime::spawn_blocking(move || {
        let folder = fs::canonicalize(path.map(PathBuf::from).unwrap_or(default))
            .map_err(|e| e.to_string())?;
        let mut entries = Vec::new();
        for entry in fs::read_dir(&folder).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let directory = entry.file_type().map_err(|e| e.to_string())?.is_dir();
            let path = entry.path();
            if directory
                || path.extension().is_some_and(|s| {
                    matches!(
                        s.to_string_lossy().to_lowercase().as_str(),
                        "pcap" | "pcapng"
                    )
                })
            {
                entries.push(Entry {
                    path: path.to_string_lossy().into(),
                    name: entry.file_name().to_string_lossy().into(),
                    directory,
                });
            }
            if entries.len() > 5000 {
                return Err("此目录条目过多，请输入更具体的子目录路径".into());
            }
        }
        entries.sort_by(|a, b| b.directory.cmp(&a.directory).then(a.name.cmp(&b.name)));
        Ok(Directory {
            path: folder.to_string_lossy().into(),
            parent: folder.parent().map(|p| p.to_string_lossy().into()),
            entries,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn summary_failure_and_annotations_preserve_source() {
        let root =
            std::env::temp_dir().join(format!("servo-library-{}", recording::timestamp_us()));
        let recorder = Recorder::new(root.clone());
        recorder.set_context("profile", serde_json::json!({"device":{"name":"original"}}));
        let status = recorder.start(16).unwrap();
        recorder.stop().unwrap();
        let folder = Path::new(&status.path);
        let before = fs::read(folder.join("session.json")).unwrap();
        save_json(
            &folder.join("annotation.json"),
            &Annotation {
                name: "现场 A".into(),
                notes: "超时".into(),
            },
        )
        .unwrap();
        assert_eq!(info(&status.path).unwrap().annotation.name, "现场 A");
        assert_eq!(before, fs::read(folder.join("session.json")).unwrap());
        fs::create_dir(folder.join("blocked.json")).unwrap();
        assert!(save_json(&folder.join("blocked.json"), &0).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
