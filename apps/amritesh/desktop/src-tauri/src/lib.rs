use tauri::{Manager, RunEvent};

mod google;
mod recorder;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(google::GoogleState::new())
        .manage(recorder::Recorder::default())
        .setup(|app| {
            if let Err(e) = recorder::recover_interrupted(app.handle()) {
                eprintln!("recovery failed: {e}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            google::google_status,
            google::google_connect,
            google::google_disconnect,
            google::list_events,
            recorder::start_recording,
            recorder::stop_recording,
            recorder::set_paused,
            recorder::recording_status,
            recorder::list_recordings,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Quitting mid-call saves the recording instead of losing it.
            if let RunEvent::Exit = event {
                let _ = recorder::finish(&app.state::<recorder::Recorder>());
            }
        });
}
