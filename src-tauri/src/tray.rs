use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let show_hide = MenuItem::with_id(app, "show_hide", "Show/Hide", true, None::<&str>)?;
    let configure_hooks =
        MenuItem::with_id(app, "configure_hooks", "Configure Hooks", true, None::<&str>)?;
    let remove_hooks =
        MenuItem::with_id(app, "remove_hooks", "Remove Hooks", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &show_hide,
            &configure_hooks,
            &remove_hooks,
            &sep1,
            &settings,
            &sep2,
            &quit,
        ],
    )?;

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("ClaudePulse")
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
            match id {
                "show_hide" => {
                    if let Some(window) = app.get_webview_window("capsule") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
                "configure_hooks" => {
                    let port = app.state::<crate::ServerPort>();
                    if let Err(e) = crate::hooks_config::install_hooks(port.0) {
                        eprintln!("Failed to configure hooks: {}", e);
                    }
                    let _ = app.emit("hooks-status-changed", true);
                }
                "remove_hooks" => {
                    if let Err(e) = crate::hooks_config::remove_hooks() {
                        eprintln!("Failed to remove hooks: {}", e);
                    }
                    let _ = app.emit("hooks-status-changed", false);
                }
                "settings" => {
                    let _ = app.emit("show-settings", ());
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}
