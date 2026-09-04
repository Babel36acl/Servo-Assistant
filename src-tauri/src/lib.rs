mod audit;
mod modbus;
mod profile;
mod runtime;

use runtime::{
    apply_parameters, batch_write_parameters, capture_parameter_snapshot,
    compare_parameter_snapshot, connect_device, disconnect_device, get_active_profile,
    get_audit_log, get_connection_status, import_profile, list_serial_ports, persist_parameters,
    read_parameters, read_statuses, write_parameter, AppState,
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            app.manage(AppState::new(data_dir.join("servo.db"))?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
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
