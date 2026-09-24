use tauri::{Manager, RunEvent};

mod google;
mod live;
mod mcp;
mod mixer;
mod oauth;
mod recorder;
mod whipscribe;

type Result<T> = std::result::Result<T, String>;

// Sentinels the UI maps to its signed-out and offline states.
const NOT_CONNECTED: &str = "not_connected";
const OFFLINE: &str = "offline";

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn net_err(e: reqwest::Error) -> String {
    if e.is_connect() || e.is_timeout() {
        OFFLINE.into()
    } else {
        e.to_string()
    }
}

fn secret(name: &str) -> Result<keyring::Entry> {
    keyring::Entry::new("com.amritesh.whipscribe-recorder", name).map_err(err)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(google::GoogleState::new())
        .manage(recorder::Recorder::default())
        .manage(whipscribe::WhipScribe::new())
        .manage(mcp::Mcp::new())
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
            recorder::rename_recording,
            whipscribe::whipscribe_status,
            whipscribe::whipscribe_connect,
            whipscribe::transcribe,
            whipscribe::job_status,
            whipscribe::transcript,
            whipscribe::delete_recording,
            whipscribe::search_transcripts,
            mcp::whipscribe_signed_in,
            mcp::whipscribe_sign_in,
            mcp::library,
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
