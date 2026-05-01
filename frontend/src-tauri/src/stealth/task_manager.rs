use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, SetWindowLongW, GetWindowLongW, GWL_EXSTYLE, WS_EX_TOOLWINDOW, WS_EX_APPWINDOW,
    ShowWindow, SW_HIDE,
};
use anyhow::Result;

/// Hides the current window from the taskbar by adding the WS_EX_TOOLWINDOW style
/// and removing WS_EX_APPWINDOW.
pub fn hide_from_window_list() {
    unsafe {
        // This is a bit simplistic as it grabs the foreground window. 
        // In a real app, we should have our own HWND.
        // For now, assuming we might be a console app or have a window.
        // If we are a console app, we can get the console window.
        
        let hwnd = windows::Win32::System::Console::GetConsoleWindow();
        
        if hwnd.0 != 0 {
            // Hide the console window completely
            // ShowWindow(hwnd, SW_HIDE); 
            
            // Or just remove from taskbar:
            let style = GetWindowLongW(hwnd, GWL_EXSTYLE);
            let new_style = (style & !WS_EX_APPWINDOW.0 as i32) | WS_EX_TOOLWINDOW.0 as i32;
            SetWindowLongW(hwnd, GWL_EXSTYLE, new_style);
        }
    }
}

pub fn hide_console() {
    unsafe {
        let hwnd = windows::Win32::System::Console::GetConsoleWindow();
        if hwnd.0 != 0 {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }
}
