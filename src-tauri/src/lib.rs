mod commands;
mod process_monitor;
mod providers;
mod session_manager;
mod settings;
mod tray;
mod webhook_server;

use providers::ProviderRegistry;
use session_manager::SessionManager;
use settings::SettingsStore;
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use tokio::sync::mpsc;
use webhook_server::WebhookServer;

pub struct ServerPort(pub u16);

/// Remove all provider integrations (hook scripts, settings entries, plugins).
/// Called with `--cleanup` during uninstall.
pub fn cleanup() {
    let registry = providers::create_registry();
    for info in registry.list() {
        if info.installed {
            if let Some(provider) = registry.get(&info.id) {
                if let Err(e) = provider.remove() {
                    eprintln!("Failed to remove {}: {}", info.id, e);
                }
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("capsule") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_sessions,
            commands::get_settings,
            commands::set_setting,
            commands::reset_settings,
            commands::get_providers,
            commands::configure_provider,
            commands::remove_provider,
            commands::get_server_port,
            commands::set_expanded,
            commands::minimize_to_tray,
            commands::remove_session,
            commands::pin_session,
            commands::unpin_session,
            commands::reorder_pinned_sessions,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Initialize settings
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data dir");
            let settings_store = SettingsStore::new(app_data_dir);
            app.manage(Mutex::new(settings_store));

            // Initialize session manager
            let session_manager = SessionManager::new();
            app.manage(session_manager.clone());

            // Initialize provider registry
            let registry = providers::create_registry();
            app.manage(Mutex::new(registry));

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

                // Check if any providers need configuration and notify frontend
                let registry = app_handle_server.state::<Mutex<ProviderRegistry>>();
                let reg = registry.lock().unwrap();
                let unconfigured: Vec<String> = reg
                    .list()
                    .iter()
                    .filter(|p| !p.installed)
                    .map(|p| p.display_name.clone())
                    .collect();
                drop(reg);
                if !unconfigured.is_empty() {
                    let _ = app_handle_server.emit("unconfigured-providers", &unconfigured);
                }
            });

            // Manage a default ServerPort (will be overwritten by the async task)
            app.manage(ServerPort(19280));

            // Process incoming events
            let session_manager_rx = app.state::<SessionManager>().inner().clone();
            let app_handle_rx = app_handle.clone();

            tauri::async_runtime::spawn(async move {
                while let Some(event) = rx.recv().await {
                    let is_stop = SessionManager::is_stop_event(&event);
                    let is_waiting = SessionManager::is_waiting_event(&event);
                    let changed = session_manager_rx.handle_event(&event);

                    if changed {
                        let sessions = session_manager_rx.get_sessions();
                        let _ = app_handle_rx.emit("sessions-changed", &sessions);
                    }

                    let sound_enabled = app_handle_rx
                        .state::<Mutex<SettingsStore>>()
                        .lock()
                        .unwrap()
                        .get()
                        .sound_on_complete;

                    if is_stop && sound_enabled {
                        let _ = app_handle_rx.emit("play-sound", ());
                    }

                    if is_waiting && sound_enabled {
                        let _ = app_handle_rx.emit("play-waiting-sound", ());
                    }
                }
            });

            // Staleness check loop
            let session_manager_stale = app.state::<SessionManager>().inner().clone();
            let app_handle_stale = app_handle.clone();

            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
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
