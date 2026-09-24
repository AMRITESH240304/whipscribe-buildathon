mod google;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(google::GoogleState::new())
        .invoke_handler(tauri::generate_handler![
            google::google_status,
            google::google_connect,
            google::google_disconnect,
            google::list_events,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
