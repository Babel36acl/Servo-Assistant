mod audit;
mod discovery;
mod ethercat;
mod master;
mod modbus;
mod network_capture;
mod pcap_file;
mod profile;
mod recorded_port;
mod recording;
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
            let recorder = recording::Recorder::new(data_dir.join("recordings"));
            app.manage(AppState::with_recorder(
                data_dir.join("servo.db"),
                recorder.clone(),
            )?);
            app.manage(master::Master::new(
                recorder.clone(),
                audit::AuditStore::open(data_dir.join("servo.db"))?,
            ));
            app.manage(network_capture::NetworkCapture::new(
                data_dir.join("captures"),
                recorder.clone(),
            ));
            app.manage(recorder);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            recording::start_recording,
            recording::stop_recording,
            recording::recording_status,
            recording::recording_sessions,
            recording::recording_page,
            pcap_file::capture_file_page,
            pcap_file::export_recording_pcap,
            ethercat::decode_ethercat,
            master::ethercat_adapters,
            master::ethercat_connect,
            master::ethercat_disconnect,
            master::ethercat_status,
            master::ethercat_profile,
            master::ethercat_read,
            master::ethercat_write,
            network_capture::start_network_capture,
            network_capture::stop_network_capture,
            network_capture::network_capture_status,
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
        .build(tauri::generate_context!())
        .expect("failed to build Servo Assistant")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                let _ = app.state::<network_capture::NetworkCapture>().stop();
                app.state::<master::Master>().close();
                let _ = app.state::<recording::Recorder>().stop();
            }
        });
}

fn portable_data_dir(executable: &std::path::Path) -> Option<std::path::PathBuf> {
    let folder = executable.parent()?;
    folder
        .join("portable.marker")
        .is_file()
        .then(|| folder.join("data"))
}
