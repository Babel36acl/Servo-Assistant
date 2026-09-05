mod audit;
mod discovery;
mod modbus;
mod profile;
mod runtime;

use runtime::{
    apply_parameters, batch_write_parameters, cancel_discovery, capture_parameter_snapshot,
    compare_parameter_snapshot, configure_communication, connect_device, disconnect_device,
    discover_device, get_active_profile, get_audit_log, get_communication_stats,
    get_connection_status, get_discovery_status, import_profile, list_serial_ports,
    persist_parameters, probe_read, read_parameters, read_statuses, write_parameter, AppState,
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let executable = std::env::current_exe()?;
            let data_dir = portable_data_dir(&executable).unwrap_or(app.path().app_data_dir()?);
            app.manage(AppState::new(data_dir.join("servo.db"))?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            discover_device,
            cancel_discovery,
            get_discovery_status,
            configure_communication,
            get_communication_stats,
            probe_read,
            import_profile,
            get_active_profile,
            list_serial_ports,
            connect_device,
            disconnect_device,
            get_connection_status,
            read_parameters,
            write_parameter,
            batch_write_parameters,
            capture_parameter_snapshot,
            compare_parameter_snapshot,
            apply_parameters,
            persist_parameters,
            read_statuses,
            get_audit_log,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Servo Assistant");
}

fn portable_data_dir(executable: &std::path::Path) -> Option<std::path::PathBuf> {
    let folder = executable.parent()?;
    folder
        .join("portable.marker")
        .is_file()
        .then(|| folder.join("data"))
}
