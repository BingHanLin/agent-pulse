use tauri::WebviewWindow;

/// Force the window to stay on top of the taskbar on Windows.
/// No-op on other platforms.
#[cfg_attr(not(target_os = "windows"), allow(unused_variables))]
pub fn force_topmost(window: &WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
        };

        unsafe {
            if let Ok(hwnd) = window.hwnd() {
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
                );
            }
        }
    }
}
