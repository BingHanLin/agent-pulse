use crate::hooks_config;
use crate::session_manager::{SessionInfo, SessionManager};
use crate::settings::{Settings, SettingsStore};
use crate::ServerPort;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub fn get_sessions(session_manager: State<'_, SessionManager>) -> Vec<SessionInfo> {
    session_manager.get_sessions()
}

#[tauri::command]
pub fn get_active_session(session_manager: State<'_, SessionManager>) -> Option<SessionInfo> {
    session_manager.get_active_session()
}

#[tauri::command]
pub fn select_session(session_manager: State<'_, SessionManager>, id: Option<String>) {
    session_manager.select_session(id);
}

#[tauri::command]
pub fn get_settings(settings_store: State<'_, Mutex<SettingsStore>>) -> Settings {
    settings_store.lock().unwrap().get_cloned()
}

#[tauri::command]
pub fn set_setting(
    app: AppHandle,
    settings_store: State<'_, Mutex<SettingsStore>>,
    key: String,
    value: String,
) -> Result<(), String> {
    let mut store = settings_store.lock().unwrap();
    store.update_field(&key, &value)?;
    let settings = store.get_cloned();
    drop(store);

    let _ = app.emit("settings-changed", &settings);

    // Reposition window if position changed
    if key == "position" {
        reposition_window(&app, &settings);
    }

    Ok(())
}

#[tauri::command]
pub fn configure_hooks(app: AppHandle, port: State<'_, ServerPort>) -> Result<(), String> {
    hooks_config::install_hooks(port.0)?;
    let _ = app.emit("hooks-status-changed", true);
    Ok(())
}

#[tauri::command]
pub fn remove_hooks(app: AppHandle) -> Result<(), String> {
    hooks_config::remove_hooks()?;
    let _ = app.emit("hooks-status-changed", false);
    Ok(())
}

#[tauri::command]
pub fn get_hook_status() -> bool {
    hooks_config::is_hooks_installed()
}

#[tauri::command]
pub fn get_server_port(port: State<'_, ServerPort>) -> u16 {
    port.0
}

#[tauri::command]
pub fn set_expanded(app: AppHandle, expanded: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("capsule") {
        let current_size = window
            .inner_size()
            .map_err(|e| format!("Failed to get window size: {}", e))?;
        let scale = window
            .scale_factor()
            .map_err(|e| format!("Failed to get scale factor: {}", e))?;

        let width = (current_size.width as f64 / scale) as u32;
        let new_height = if expanded { 400 } else { 52 };

        let size = tauri::LogicalSize::new(width, new_height);
        window
            .set_size(size)
            .map_err(|e| format!("Failed to resize window: {}", e))?;
    }
    Ok(())
}

fn reposition_window(app: &AppHandle, settings: &Settings) {
    use crate::settings::PanelPosition;

    if let Some(window) = app.get_webview_window("capsule") {
        if let Ok(monitor) = window.primary_monitor() {
            if let Some(monitor) = monitor {
                let monitor_size = monitor.size();
                let scale = monitor.scale_factor();
                let monitor_w = monitor_size.width as f64 / scale;
                let monitor_h = monitor_size.height as f64 / scale;

                let (x, y) = match settings.position {
                    PanelPosition::TopCenter => ((monitor_w - 360.0) / 2.0, 8.0),
                    PanelPosition::BottomLeft => (16.0, monitor_h - 60.0),
                    PanelPosition::BottomRight => (monitor_w - 376.0, monitor_h - 60.0),
                };

                let _ = window.set_position(tauri::LogicalPosition::new(x, y));
            }
        }
    }
}
