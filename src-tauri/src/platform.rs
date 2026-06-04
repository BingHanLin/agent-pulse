use tauri::WebviewWindow;

/// Force the window to stay on top of the taskbar on Windows.
/// No-op on other platforms.
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

/// Set the window's opacity (0.0–1.0) via a layered window on Windows.
/// No-op on other platforms.
pub fn set_window_alpha(window: &WebviewWindow, alpha: f64) {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::COLORREF;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetLayeredWindowAttributes, SetWindowLongPtrW, GWL_EXSTYLE,
            LWA_ALPHA, WS_EX_LAYERED,
        };

        let alpha = alpha.clamp(0.0, 1.0);
        let byte = (alpha * 255.0).round() as u8;

        unsafe {
            if let Ok(hwnd) = window.hwnd() {
                // Ensure the window has the layered style before setting alpha.
                let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                let layered = WS_EX_LAYERED.0 as isize;
                if ex_style & layered == 0 {
                    SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | layered);
                }
                let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), byte, LWA_ALPHA);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, alpha);
    }
}
