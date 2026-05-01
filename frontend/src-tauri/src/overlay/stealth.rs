use windows::{
    core::*,
    Win32::{
        Foundation::*,
        UI::WindowsAndMessaging::*,
        UI::Input::KeyboardAndMouse::{
            RegisterHotKey, MOD_ALT, MOD_SHIFT, MOD_CONTROL, 
            VK_LEFT, VK_RIGHT, VK_UP, VK_DOWN,
            VK_ADD, VK_SUBTRACT, VK_OEM_PLUS, VK_OEM_MINUS
        },
    },
};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, Emitter};

// =============================================================================
// 🎹 HOTKEY ID CONSTANTS
// =============================================================================
const HOTKEY_SHIFT_Q: usize = 1100;     // Send transcription to AI NOW
const HOTKEY_SHIFT_A: usize = 1101;     // Clear ALL
const HOTKEY_SHIFT_S: usize = 1200;     // Capture screenshot
const HOTKEY_SHIFT_D: usize = 1201;     // Delete all screenshots
const HOTKEY_SHIFT_F: usize = 1202;     // Send/Flash to AI
const HOTKEY_SHIFT_M: usize = 1203;     // Cycle AI model
const HOTKEY_SHIFT_U: usize = 1300;     // Scroll UP
const HOTKEY_SHIFT_N: usize = 1301;     // Scroll DOWN
const HOTKEY_SHIFT_T: usize = 1302;     // Toggle visibility
const HOTKEY_SHIFT_P: usize = 1303;     // Toggle (legacy)

const HOTKEY_ALT_SHIFT_TOGGLE: usize = 1001;
const HOTKEY_ALT_SHIFT_TEST: usize = 1002;
const HOTKEY_ALT_SHIFT_S: usize = 1003;
const HOTKEY_ALT_SHIFT_A: usize = 1004;
const HOTKEY_ALT_SHIFT_D: usize = 1005;
const HOTKEY_ALT_SHIFT_LEFT: usize = 1006;
const HOTKEY_ALT_SHIFT_RIGHT: usize = 1007;
const HOTKEY_ALT_SHIFT_UP: usize = 1008;
const HOTKEY_ALT_SHIFT_DOWN: usize = 1009;
const HOTKEY_ALT_SHIFT_W: usize = 1010;
const HOTKEY_ALT_SHIFT_X: usize = 1011;
const HOTKEY_ALT_SHIFT_PLUS: usize = 1012;
const HOTKEY_ALT_SHIFT_MINUS: usize = 1013;
const HOTKEY_ALT_SHIFT_OEM_PLUS: usize = 1014;
const HOTKEY_ALT_SHIFT_OEM_MINUS: usize = 1015;
const HOTKEY_ALT_SHIFT_K: usize = 1016;
const HOTKEY_ALT_SHIFT_I: usize = 1017;
const HOTKEY_ALT_SHIFT_B: usize = 1018;
const HOTKEY_ALT_SHIFT_M: usize = 1019;
const HOTKEY_ALT_SHIFT_V: usize = 1020;
const HOTKEY_ALT_SHIFT_C: usize = 1021;
const HOTKEY_ALT_SHIFT_Z: usize = 1022;

// Ctrl+Arrow for moving the panel
const HOTKEY_CTRL_LEFT: usize = 1400;
const HOTKEY_CTRL_RIGHT: usize = 1401;
const HOTKEY_CTRL_UP: usize = 1402;
const HOTKEY_CTRL_DOWN: usize = 1403;

static OVERLAY_VISIBLE: AtomicBool = AtomicBool::new(true);
// Track window position so move_overlay can calculate new position
static WIN_X: AtomicI32 = AtomicI32::new(-1);
static WIN_Y: AtomicI32 = AtomicI32::new(-1);

lazy_static::lazy_static! {
    static ref TAURI_APP: Mutex<Option<AppHandle>> = Mutex::new(None);
    static ref LAST_AI_RESPONSE: Mutex<String> = Mutex::new(String::new());
    static ref LIVE_TRANSCRIPTION: Mutex<String> = Mutex::new(String::new());
    static ref OVERLAY_TEXT: Mutex<String> = Mutex::new(String::from("📡 SM Active\n\n🔒 Stealth: ON\n👁️ You only"));
}

pub fn set_app_handle(handle: AppHandle) {
    if let Ok(mut app) = TAURI_APP.lock() {
        *app = Some(handle);
    }
}

// Helper to emit events to frontend
fn emit_event(event: &str, payload: &str) {
    if let Ok(app_guard) = TAURI_APP.lock() {
        if let Some(app) = app_guard.as_ref() {
            let _ = app.emit(event, payload);
        }
    }
}

pub fn init() {
    crate::log_info!("[SUCCESS] Starting ENHANCED stealth hotkey listener.");
    unsafe {
        register_and_listen_hotkeys();
    }
}

pub fn set_overlay_text(text: String) {
    if let Ok(mut overlay_text) = OVERLAY_TEXT.try_lock() {
        *overlay_text = text.clone();
    }
    emit_event("overlay-text-update", &text);
}

pub fn set_status_message(text: String) {
    emit_event("status-message-update", &text);
}

pub fn append_ai_response(text: &str) {
    let mut full_text = String::new();
    if let Ok(mut last_resp) = LAST_AI_RESPONSE.try_lock() {
        if text.is_empty() { return; }
        last_resp.push_str(text);
        full_text = last_resp.clone();
        if let Ok(mut overlay_text) = OVERLAY_TEXT.try_lock() {
            *overlay_text = full_text.clone();
        }
    }
    if !full_text.is_empty() {
        emit_event("ai-response-update", &full_text);
    }
}

pub fn force_ai_response_update(text: &str) {
    if let Ok(mut last_resp) = LAST_AI_RESPONSE.try_lock() {
        *last_resp = text.to_string();
        if let Ok(mut overlay_text) = OVERLAY_TEXT.try_lock() {
            *overlay_text = text.to_string();
        }
    }
    emit_event("ai-response-update", text);
}

pub fn reset_ai_response() {
    if let Ok(mut last_resp) = LAST_AI_RESPONSE.try_lock() {
        last_resp.clear();
    }
    if let Ok(mut overlay_text) = OVERLAY_TEXT.try_lock() {
        overlay_text.clear();
    }
    emit_event("ai-response-update", "");
}

pub fn clear_all_buffers() {
    for _ in 0..10 {
        if let Ok(mut last_resp) = LAST_AI_RESPONSE.try_lock() {
            last_resp.clear();
            break;
        }
    }
    for _ in 0..10 {
        if let Ok(mut trans) = LIVE_TRANSCRIPTION.try_lock() {
            trans.clear();
            break;
        }
    }
    for _ in 0..10 {
        if let Ok(mut overlay) = OVERLAY_TEXT.try_lock() {
            overlay.clear();
            break;
        }
    }
    emit_event("clear-all-buffers", "");
}

pub fn set_live_transcription_snapshot(snapshot: String) {
    if let Ok(mut trans) = LIVE_TRANSCRIPTION.try_lock() {
        *trans = snapshot.clone();
    }
    emit_event("live-transcription-update", &snapshot);
}

pub fn set_live_transcription(text: &str) {
    set_live_transcription_snapshot(text.to_string());
}

pub fn get_live_transcription() -> String {
    if let Ok(trans) = LIVE_TRANSCRIPTION.try_lock() {
        trans.clone()
    } else {
        String::new()
    }
}

pub fn toggle_visibility() {
    let is_visible = OVERLAY_VISIBLE.load(Ordering::SeqCst);
    OVERLAY_VISIBLE.store(!is_visible, Ordering::SeqCst);
    
    if let Ok(app_guard) = TAURI_APP.lock() {
        if let Some(app) = app_guard.as_ref() {
            if let Some(window) = app.get_webview_window("main") {
                if is_visible {
                    let _ = window.hide();
                } else {
                    let _ = window.show();
                }
            }
        }
    }
}

pub fn move_overlay(dx: i32, dy: i32) {
    if let Ok(app_guard) = TAURI_APP.lock() {
        if let Some(app) = app_guard.as_ref() {
            if let Some(window) = app.get_webview_window("main") {
                // Get current position
                let pos = window.outer_position().unwrap_or_default();
                let new_x = pos.x + dx;
                let new_y = pos.y + dy;
                WIN_X.store(new_x, Ordering::SeqCst);
                WIN_Y.store(new_y, Ordering::SeqCst);
                let _ = window.set_position(tauri::Position::Physical(
                    tauri::PhysicalPosition { x: new_x, y: new_y }
                ));
            }
        }
    }
}

pub fn resize_overlay(dw: i32, dh: i32) {
    if let Ok(app_guard) = TAURI_APP.lock() {
        if let Some(app) = app_guard.as_ref() {
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(size) = window.outer_size() {
                    let new_w = ((size.width as i32) + dw).max(280) as u32;
                    let new_h = ((size.height as i32) + dh).max(160) as u32;
                    let _ = window.set_size(tauri::Size::Physical(
                        tauri::PhysicalSize { width: new_w, height: new_h }
                    ));
                }
            }
        }
    }
}

pub fn scroll_overlay(delta: i32) {
    emit_event("scroll-event", &delta.to_string());
}

unsafe fn register_and_listen_hotkeys() {
    let hwnd = HWND(0); // Thread message queue

    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_Q as i32, MOD_SHIFT, 0x51);
    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_A as i32, MOD_SHIFT, 0x41);
    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_S as i32, MOD_SHIFT, 0x53);
    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_D as i32, MOD_SHIFT, 0x44);
    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_F as i32, MOD_SHIFT, 0x46);
    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_M as i32, MOD_SHIFT, 0x4D);
    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_U as i32, MOD_SHIFT, 0x55);
    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_N as i32, MOD_SHIFT, 0x4E);
    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_T as i32, MOD_SHIFT, 0x54);
    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_P as i32, MOD_SHIFT, 0x50);
    
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_TEST as i32, MOD_ALT | MOD_SHIFT, 0x54);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_S as i32, MOD_ALT | MOD_SHIFT, 0x53);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_A as i32, MOD_ALT | MOD_SHIFT, 0x41);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_D as i32, MOD_ALT | MOD_SHIFT, 0x44);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_LEFT as i32, MOD_ALT | MOD_SHIFT, VK_LEFT.0 as u32);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_RIGHT as i32, MOD_ALT | MOD_SHIFT, VK_RIGHT.0 as u32);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_UP as i32, MOD_ALT | MOD_SHIFT, VK_UP.0 as u32);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_DOWN as i32, MOD_ALT | MOD_SHIFT, VK_DOWN.0 as u32);
    // Ctrl+Arrows for moving the panel
    let _ = RegisterHotKey(hwnd, HOTKEY_CTRL_LEFT as i32, MOD_CONTROL, VK_LEFT.0 as u32);
    let _ = RegisterHotKey(hwnd, HOTKEY_CTRL_RIGHT as i32, MOD_CONTROL, VK_RIGHT.0 as u32);
    let _ = RegisterHotKey(hwnd, HOTKEY_CTRL_UP as i32, MOD_CONTROL, VK_UP.0 as u32);
    let _ = RegisterHotKey(hwnd, HOTKEY_CTRL_DOWN as i32, MOD_CONTROL, VK_DOWN.0 as u32);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_W as i32, MOD_ALT | MOD_SHIFT, 0x57);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_X as i32, MOD_ALT | MOD_SHIFT, 0x58);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_PLUS as i32, MOD_ALT | MOD_SHIFT, VK_ADD.0 as u32);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_MINUS as i32, MOD_ALT | MOD_SHIFT, VK_SUBTRACT.0 as u32);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_OEM_PLUS as i32, MOD_ALT | MOD_SHIFT, VK_OEM_PLUS.0 as u32);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_OEM_MINUS as i32, MOD_ALT | MOD_SHIFT, VK_OEM_MINUS.0 as u32);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_K as i32, MOD_ALT | MOD_SHIFT, 0x4B);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_I as i32, MOD_ALT | MOD_SHIFT, 0x49);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_B as i32, MOD_ALT | MOD_SHIFT, 0x42);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_M as i32, MOD_ALT | MOD_SHIFT, 0x4D);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_V as i32, MOD_ALT | MOD_SHIFT, 0x56);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_C as i32, MOD_ALT | MOD_SHIFT, 0x43);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_Z as i32, MOD_ALT | MOD_SHIFT, 0x5A);

    crate::log_info!("[HOTKEY] All hotkeys registered successfully");

    let mut message = MSG::default();
    crate::log_info!("Entering Hotkey Message Loop");
    while GetMessageA(&mut message, HWND(0), 0, 0).into() {
        if message.message == WM_HOTKEY {
            let id = message.wParam.0;
            crate::log_info!("WM_HOTKEY received: ID {}", id);
            
            // Re-route hotkey logic directly here
            match id {
                HOTKEY_ALT_SHIFT_TOGGLE => { toggle_visibility(); },
                HOTKEY_ALT_SHIFT_TEST => { set_overlay_text("Test!".to_string()); },
                HOTKEY_ALT_SHIFT_S | HOTKEY_SHIFT_S => { 
                    std::thread::spawn(|| {
                        crate::capture::take_screenshot();
                        let count = crate::capture::get_screenshot_count();
                        set_status_message(format!("📸 Screenshot #{} taken!", count));
                    });
                },
                HOTKEY_ALT_SHIFT_A => {
                    let count = crate::capture::get_screenshot_count();
                    if count > 0 {
                        set_status_message(format!("🔄 Analyzing {} snapshots...", count));
                        std::thread::spawn(|| {
                            let screenshots = crate::capture::get_all_screenshots();
                            reset_ai_response();
                            crate::ai::ask_ai_with_images("Analyze these images", screenshots, |streaming_text| {
                                if let Ok(mut last_resp) = LAST_AI_RESPONSE.try_lock() {
                                    *last_resp = streaming_text.clone();
                                }
                                append_ai_response(&streaming_text);
                            });
                        });
                    } else {
                        set_overlay_text("❌ No screenshots!\nPress Alt+Shift+S first".to_string());
                    }
                },
                HOTKEY_ALT_SHIFT_D | HOTKEY_SHIFT_D => {
                    std::thread::spawn(|| {
                        crate::capture::delete_all_screenshots();
                        set_status_message("🗑️ All screenshots deleted!".to_string());
                    });
                },
                HOTKEY_ALT_SHIFT_K => {
                    crate::log_info!("Kill hotkey - Exiting application");
                    set_overlay_text("Exiting...".to_string());
                    crate::request_shutdown();
                    if let Ok(app_guard) = TAURI_APP.lock() {
                        if let Some(app) = app_guard.as_ref() {
                            app.exit(0);
                        }
                    }
                    break;
                },
                HOTKEY_ALT_SHIFT_I => {
                    if crate::autotype::is_typing() {
                        crate::autotype::stop_typing();
                        set_overlay_text("Auto-type stopped.".to_string());
                    } else {
                        let text_to_type = match LAST_AI_RESPONSE.lock() {
                            Ok(guard) => guard.clone(),
                            Err(poisoned) => poisoned.into_inner().clone(),
                        };
                        if !text_to_type.is_empty() {
                            set_overlay_text("Auto-typing started...".to_string());
                            crate::autotype::start_typing(text_to_type);
                        } else {
                            set_overlay_text("No AI response to type!".to_string());
                        }
                    }
                },
                HOTKEY_ALT_SHIFT_B => {
                    let new_state = crate::autotype::toggle_auto_bracket_mode();
                    if new_state {
                        set_overlay_text("🔧 Auto-Bracket: ON\n(Delete key will remove auto-inserted brackets)".to_string());
                    } else {
                        set_overlay_text("🔧 Auto-Bracket: OFF\n(No auto-bracket compensation)".to_string());
                    }
                },
                HOTKEY_ALT_SHIFT_M | HOTKEY_SHIFT_M => {
                    std::thread::spawn(|| {
                        let new_model = crate::ai::cycle_model();
                        set_status_message(format!("🤖 AI Model: {}", new_model.name()));
                    });
                },
                HOTKEY_ALT_SHIFT_V => { 
                    if !crate::audio::is_live_streaming() {
                         set_overlay_text("🚀 Connecting to Live API...".to_string());
                         reset_ai_response(); 
                         std::thread::spawn(|| {
                             crate::log_info!("Live session thread started...");
                             let result = crate::TOKIO_RT.block_on(async {
                                 crate::ai::live_client::start_live_session(|text| {
                                     append_ai_response(&text);
                                 }).await
                             });
                             match result {
                                 Ok(_) => crate::log_info!("Live session ended gracefully"),
                                 Err(e) => {
                                     crate::log_error!("Live session error: {}", e);
                                     set_overlay_text(format!("❌ Error: {}", e));
                                 }
                             }
                         });
                    } else {
                        set_overlay_text("⚠️ Already connected!".to_string());
                    }
                },
                HOTKEY_ALT_SHIFT_C => { 
                    if crate::audio::is_live_streaming() {
                        crate::audio::IS_LIVE_STREAMING.store(false, Ordering::SeqCst);
                        set_overlay_text("🛑 Closing connection...".to_string());
                    } else {
                        set_overlay_text("❌ Not connected.".to_string());
                    }
                },
                HOTKEY_ALT_SHIFT_Z => { 
                     reset_ai_response();
                     set_overlay_text("".to_string());
                },
                HOTKEY_SHIFT_Q => { 
                    crate::log_info!("⚡ Shift+Q pressed - triggering AI NOW");
                    set_overlay_text("⚡ Requesting Answer...".to_string());
                    crate::ai::live_client::trigger_answer_now();
                },
                HOTKEY_SHIFT_A => { 
                    crate::log_info!("🧹 Shift+A pressed - CLEARING ALL");
                    set_status_message("🧹 Cleared.".to_string());
                    crate::ai::live_client::force_clear_buffers();
                },
                HOTKEY_SHIFT_F => { 
                    set_status_message("⏳ Analyzing screen...".to_string());
                    reset_ai_response(); 
                    crate::ai::live_client::trigger_screenshot_analysis(|text| {
                        append_ai_response(&text);
                    });
                },
                HOTKEY_SHIFT_U => { scroll_overlay(-100); },
                HOTKEY_SHIFT_N => { scroll_overlay(100); },
                HOTKEY_SHIFT_T | HOTKEY_SHIFT_P => { toggle_visibility(); },
                // Arrow keys: move the panel
                HOTKEY_CTRL_LEFT | HOTKEY_ALT_SHIFT_LEFT   => { move_overlay(-20, 0); },
                HOTKEY_CTRL_RIGHT | HOTKEY_ALT_SHIFT_RIGHT => { move_overlay(20, 0); },
                HOTKEY_CTRL_UP | HOTKEY_ALT_SHIFT_UP       => { move_overlay(0, -20); },
                HOTKEY_CTRL_DOWN | HOTKEY_ALT_SHIFT_DOWN   => { move_overlay(0, 20); },
                // Width resize: Alt+Shift+Plus/Minus
                HOTKEY_ALT_SHIFT_PLUS | HOTKEY_ALT_SHIFT_OEM_PLUS => {
                    resize_overlay(40, 0);   
                },
                HOTKEY_ALT_SHIFT_MINUS | HOTKEY_ALT_SHIFT_OEM_MINUS => {
                    resize_overlay(-40, 0);  
                },
                // Height resize: Alt+Shift+W/X
                HOTKEY_ALT_SHIFT_W => {
                    resize_overlay(0, 40);   
                },
                HOTKEY_ALT_SHIFT_X => {
                    resize_overlay(0, -40);  
                },
                _ => {}
            }
        }
    }
    crate::log_info!("Exiting Hotkey Message Loop");
}
