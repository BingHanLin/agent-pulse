mod commands;
mod hooks_config;
mod session_manager;
mod settings;
mod tray;
mod webhook_server;

use session_manager::SessionManager;
use settings::SettingsStore;
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use tokio::sync::mpsc;
use webhook_server::WebhookServer;

pub struct ServerPort(pub u16);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_sessions,
            commands::get_active_session,
            commands::select_session,
            commands::get_settings,
            commands::set_setting,
            commands::configure_hooks,
            commands::remove_hooks,
            commands::get_hook_status,
            commands::get_server_port,
            commands::set_expanded,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Initialize settings
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data dir");
            let settings_store = SettingsStore::new(app_data_dir);
            let sound_enabled = settings_store.get().sound_on_complete;
            app.manage(Mutex::new(settings_store));

            // Initialize session manager
            let session_manager = SessionManager::new();
            app.manage(session_manager.clone());

            // Start webhook server
            let (tx, mut rx) = mpsc::unbounded_channel();
            let app_handle_server = app_handle.clone();

            tauri::async_runtime::spawn(async move {
                let server = match WebhookServer::start(tx).await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Failed to start webhook server: {}", e);
                        return;
                    }
                };

                // Store the port
                app_handle_server.manage(ServerPort(server.port()));

                // Auto-configure hooks if not already installed
                if !hooks_config::is_hooks_installed() {
                    if let Err(e) = hooks_config::install_hooks(server.port()) {
                        eprintln!("Failed to auto-configure hooks: {}", e);
                    }
                }
            });

            // Manage a default ServerPort (will be overwritten by the async task)
            // This is needed so State<ServerPort> is available immediately
            app.manage(ServerPort(19280));

            // Process incoming events
            let session_manager_rx = app.state::<SessionManager>().inner().clone();
            let app_handle_rx = app_handle.clone();

            tauri::async_runtime::spawn(async move {
                while let Some(event) = rx.recv().await {
                    let is_stop = SessionManager::is_stop_event(&event);
                    let changed = session_manager_rx.handle_event(&event);

                    if changed {
                        let sessions = session_manager_rx.get_sessions();
                        let _ = app_handle_rx.emit("sessions-changed", &sessions);
                    }

                    if is_stop && sound_enabled {
                        let _ = app_handle_rx.emit("play-sound", ());
                    }
                }
            });

            // Staleness check loop
            let session_manager_stale = app.state::<SessionManager>().inner().clone();
            let app_handle_stale = app_handle.clone();

            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    if session_manager_stale.check_staleness() {
                        let sessions = session_manager_stale.get_sessions();
                        let _ = app_handle_stale.emit("sessions-changed", &sessions);
                    }
                }
            });

            // Setup system tray
            if let Err(e) = tray::setup_tray(app.handle()) {
                eprintln!("Failed to setup tray: {}", e);
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
