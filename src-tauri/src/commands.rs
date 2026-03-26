use crate::providers::{ProviderInfo, ProviderRegistry};
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

    Ok(())
}

#[tauri::command]
pub fn reset_settings(
    app: AppHandle,
    settings_store: State<'_, Mutex<SettingsStore>>,
) -> Result<(), String> {
    let mut store = settings_store.lock().unwrap();
    store.reset();
    let settings = store.get_cloned();
    drop(store);
    let _ = app.emit("settings-changed", &settings);
    Ok(())
}

#[tauri::command]
pub fn get_providers(registry: State<'_, Mutex<ProviderRegistry>>) -> Vec<ProviderInfo> {
    registry.lock().unwrap().list()
}

#[tauri::command]
pub fn configure_provider(
    app: AppHandle,
    registry: State<'_, Mutex<ProviderRegistry>>,
    port: State<'_, ServerPort>,
    id: String,
) -> Result<(), String> {
    let reg = registry.lock().unwrap();
    let provider = reg
        .get(&id)
        .ok_or_else(|| format!("Unknown provider: {}", id))?;
    provider.install(port.0)?;
    drop(reg);
    let providers = registry.lock().unwrap().list();
    let _ = app.emit("providers-changed", &providers);
    Ok(())
}

#[tauri::command]
pub fn remove_provider(
    app: AppHandle,
    registry: State<'_, Mutex<ProviderRegistry>>,
    id: String,
) -> Result<(), String> {
    let reg = registry.lock().unwrap();
    let provider = reg
        .get(&id)
        .ok_or_else(|| format!("Unknown provider: {}", id))?;
    provider.remove()?;
    drop(reg);
    let providers = registry.lock().unwrap().list();
    let _ = app.emit("providers-changed", &providers);
    Ok(())
}

#[tauri::command]
pub fn get_server_port(port: State<'_, ServerPort>) -> u16 {
    port.0
}

#[tauri::command]
pub fn set_expanded(app: AppHandle, height: u32) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("capsule") {
        let current_size = window
            .inner_size()
            .map_err(|e| format!("Failed to get window size: {}", e))?;
        let scale = window
            .scale_factor()
            .map_err(|e| format!("Failed to get scale factor: {}", e))?;

        let width = (current_size.width as f64 / scale) as u32;

        let size = tauri::LogicalSize::new(width, height);
        window
            .set_size(size)
            .map_err(|e| format!("Failed to resize window: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn remove_session(
    app: AppHandle,
    session_manager: State<'_, SessionManager>,
    session_id: String,
) -> Result<(), String> {
    session_manager.remove_session(&session_id)?;
    let sessions = session_manager.get_sessions();
    let _ = app.emit("sessions-changed", &sessions);
    Ok(())
}

#[tauri::command]
pub fn minimize_to_tray(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("capsule") {
        window
            .hide()
            .map_err(|e| format!("Failed to hide window: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn close_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn pin_session(
    app: AppHandle,
    session_manager: State<'_, SessionManager>,
    session_id: String,
) -> Result<(), String> {
    session_manager.pin_session(&session_id)?;
    let sessions = session_manager.get_sessions();
    let _ = app.emit("sessions-changed", &sessions);
    Ok(())
}

#[tauri::command]
pub fn unpin_session(
    app: AppHandle,
    session_manager: State<'_, SessionManager>,
    session_id: String,
) -> Result<(), String> {
    session_manager.unpin_session(&session_id)?;
    let sessions = session_manager.get_sessions();
    let _ = app.emit("sessions-changed", &sessions);
    Ok(())
}

#[tauri::command]
pub fn reorder_pinned_sessions(
    app: AppHandle,
    session_manager: State<'_, SessionManager>,
    ordered_ids: Vec<String>,
) -> Result<(), String> {
    session_manager.reorder_pinned_sessions(ordered_ids)?;
    let sessions = session_manager.get_sessions();
    let _ = app.emit("sessions-changed", &sessions);
    Ok(())
}
