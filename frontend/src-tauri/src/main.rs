// 🛡️ CRITICAL: This prevents console window from ever appearing
#![windows_subsystem = "windows"]

//! =============================================================================
//! INTERVIEW HELPER - Direct EXE Edition
//! =============================================================================
//! 
//! A standalone application for interview assistance with:
//!   - Live audio transcription (AssemblyAI)
//!   - AI-powered answer generation (Google Gemini)
//!   - Screenshot analysis
//!   - Stealth overlay (invisible to screen capture)
//!
//! USAGE:
//!   1. Run interview-helper.exe
//!   2. Use hotkeys to interact:
//!      - Alt+Shift+V: Start live voice mode
//!      - Shift+Q: Send transcription to AI
//!      - Shift+S: Take screenshot
//!      - Shift+F: Analyze screenshots
//!      - Shift+T: Toggle overlay visibility
//!      - Alt+Shift+C: Stop voice mode
//!      - Alt+Shift+K: Exit application
//!
//! =============================================================================

use std::sync::atomic::{AtomicBool, Ordering};

mod stealth;
mod capture;
mod ai;
mod overlay;
mod network;
mod logger;
mod autotype;
mod audio;
mod services;
mod license;
mod config;


// =============================================================================
// APPLICATION STATE
// =============================================================================

/// Signals threads to stop gracefully
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

lazy_static::lazy_static! {
    /// Tokio runtime - works perfectly in EXE context (no DLL loader lock issues!)
    pub static ref TOKIO_RT: tokio::runtime::Runtime =
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(8)  // 8 threads: Audio, WebSocket R/W, Ping, AI Gen, Buffer, UI, spare
            .enable_all()
            .thread_name("interview-helper-async")
            .build()
            .expect("Failed to create Tokio runtime");
}

/// Check if shutdown has been requested
pub fn is_shutdown_requested() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::SeqCst)
}

/// Request graceful shutdown
pub fn request_shutdown() {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}

// =============================================================================
// MAIN ENTRY POINT
// =============================================================================

fn main() {
    // Hide console window for stealth (comment out for debugging)
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Console::GetConsoleWindow;
        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
        let console = GetConsoleWindow();
        if console.0 != 0 {
            let _ = ShowWindow(console, SW_HIDE);
        }
    }

    // Initialize logging
    let log_path = std::env::temp_dir().join("interview_helper.log");
    let log_path_str = log_path.to_string_lossy().to_string();
    logger::init(&log_path_str);
    
    log_info!("=== INTERVIEW HELPER v0.2.0 - Direct EXE ===");
    log_info!("Interview Helper started - PID: {}", std::process::id());
    log_info!("Log file: {}", log_path_str);

    // Set up panic hook
    std::panic::set_hook(Box::new(|info| {
        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Unknown panic",
            },
        };
        let location = info.location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown".to_string());
        log_error!("PANIC at {}: {}", location, msg);
        logger::flush();
    }));

    // ==========================================================================
    // LICENSE CHECK - REMOVED
    // ==========================================================================
    log_info!("[OK] License check bypassed");

    // ==========================================================================
    // INTERVIEW SETUP - Shows EVERY time before interview
    // User can update profile, resume, target role, and API keys
    // ==========================================================================
    log_info!("[OK] Interview setup bypassed - using React / .env");

    // ==========================================================================
    // API KEYS CHECK - Must have API keys configured
    // ==========================================================================
    if !config::are_api_keys_configured() {
        log_error!("API keys not configured!");
        // Could show a minimal API keys dialog here
        // For now, we continue and let validate_api_keys show the warning
    }

    // Validate API keys on startup
    validate_api_keys();

    // Initialize all modules
    log_info!("Initializing modules...");
    
    capture::init();
    log_info!("[OK] Screenshot capture ready");
    
    ai::init();
    log_info!("[OK] AI module ready");
    
    network::init();
    log_info!("[OK] Network module ready");

    // Start the overlay / hotkey manager in a background thread
    log_info!("Starting background hotkey listener...");
    std::thread::spawn(|| {
        overlay::init();
    });
    
    // Start Tauri application
    log_info!("Starting Tauri React UI...");
    tauri::Builder::default()
        .setup(|app| {
            // Setup Tauri window (Make it transparent, topmost, stealth)
            use tauri::Manager;
            let window = app.get_webview_window("main").unwrap();
            
            #[cfg(windows)]
            {
                use windows::Win32::Foundation::HWND;
                use windows::Win32::UI::WindowsAndMessaging::{
                    SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
                    HWND_TOPMOST, SWP_NOACTIVATE, SetWindowPos,
                    GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN,
                };
                
                let hwnd = HWND(window.hwnd().unwrap().0 as isize);
                unsafe {
                    // Get real screen dimensions at runtime
                    let screen_w = GetSystemMetrics(SM_CXSCREEN);
                    let screen_h = GetSystemMetrics(SM_CYSCREEN);
                    
                    let win_w: i32 = 460;
                    let win_h: i32 = 540;
                    // Position: bottom-right, 20px from edge, 60px from bottom (above taskbar)
                    let win_x = screen_w - win_w - 20;
                    let win_y = screen_h - win_h - 60;
                    
                    // Position and size the window correctly
                    let _ = SetWindowPos(
                        hwnd, HWND_TOPMOST,
                        win_x, win_y, win_w, win_h,
                        SWP_NOACTIVATE,
                    );

                    // Stealth: invisible to OBS / screen recorders
                    let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
                }
            }
            
            // Pass the app handle to overlay so it can emit events
            overlay::set_app_handle(app.handle().clone());
            
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            config::user_profile::load_user_profile,
            config::user_profile::save_user_profile,
            config::user_profile::update_interview_context,
            ai::live_client::get_use_photo_context,
            ai::live_client::set_use_photo_context,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    
    // If we get here, Tauri has exited
    log_info!("Application shutting down...");
    request_shutdown();
    
    // Give async tasks time to cleanup
    std::thread::sleep(std::time::Duration::from_millis(500));
    
    log_info!("Goodbye!");
    logger::flush();
}

/// Validate that required API keys are configured
fn validate_api_keys() {
    let mut missing_keys = Vec::new();
    
    if ai::ASSEMBLY_AI_KEY.is_empty() {
        missing_keys.push("ASSEMBLY_AI_KEY");
    }
    if ai::GOOGLE_API_KEY.is_empty() {
        missing_keys.push("GOOGLE_API_KEY");
    }
    
    if !missing_keys.is_empty() {
        log_error!("WARNING: Missing API keys: {:?}", missing_keys);
        log_error!("Please create a .env file with the required keys.");
    } else {
        log_info!("[OK] All API keys configured");
    }
}
