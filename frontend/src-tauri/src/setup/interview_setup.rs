//! Interview Setup UI - Simplified
//! 
//! Shows EVERY time before starting:
//! - Your name/intro
//! - Target role
//! - Resume (AI extracts skills, experience from this)
//!
//! Clean, minimal Premium Dark UI

use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleA,
        UI::WindowsAndMessaging::*,
    },
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::cell::RefCell;

use crate::config::{load_user_profile, save_user_profile, load_api_keys, save_api_keys, UserProfile, ApiKeys};
use crate::config::user_profile::ResponseStyle;

// =============================================================================
// COLORS - Premium Dark Theme
// =============================================================================
const COLOR_BG: u32 = 0x001A1A1A;            // Dark background
const COLOR_CARD: u32 = 0x00262626;          // Card/header
const COLOR_ACCENT: u32 = 0x0000D4AA;        // Teal accent
const COLOR_TEXT: u32 = 0x00FFFFFF;          // White text
const COLOR_TEXT_DIM: u32 = 0x00888888;      // Dim text
const COLOR_INPUT: u32 = 0x00333333;         // Input background

// =============================================================================
// STATE
// =============================================================================

static SETUP_OK: AtomicBool = AtomicBool::new(false);
static SHOW_API: AtomicBool = AtomicBool::new(false);

lazy_static::lazy_static! {
    static ref BRUSH_BG: Mutex<HBRUSH> = Mutex::new(HBRUSH::default());
    static ref BRUSH_CARD: Mutex<HBRUSH> = Mutex::new(HBRUSH::default());
    static ref BRUSH_ACCENT: Mutex<HBRUSH> = Mutex::new(HBRUSH::default());
    static ref BRUSH_INPUT: Mutex<HBRUSH> = Mutex::new(HBRUSH::default());
    static ref FONT_TITLE: Mutex<HFONT> = Mutex::new(HFONT::default());
    static ref FONT_BODY: Mutex<HFONT> = Mutex::new(HFONT::default());
    static ref FONT_SMALL: Mutex<HFONT> = Mutex::new(HFONT::default());
}

thread_local! {
    static DATA: RefCell<SetupData> = RefCell::new(SetupData::default());
}

#[derive(Default, Clone)]
struct SetupData {
    name: String,
    target_role: String,
    resume: String,
    google_key: String,
    groq_key: String,
    nvidia_key: String,
    assembly_key: String,
}

// Control IDs
const ID_NAME: i32 = 101;
const ID_ROLE: i32 = 102;
const ID_RESUME: i32 = 103;
const ID_GOOGLE: i32 = 104;
const ID_GROQ: i32 = 106;
const ID_NVIDIA: i32 = 107;
const ID_ASSEMBLY: i32 = 105;
const ID_START: i32 = 201;
const ID_API_BTN: i32 = 202;
const ID_CANCEL: i32 = 203;

const ES_AUTOHSCROLL: u32 = 0x0080;
const ES_MULTILINE: u32 = 0x0004;
const ES_AUTOVSCROLL: u32 = 0x0040;
const ES_PASSWORD: u32 = 0x0020;

// =============================================================================
// PUBLIC
// =============================================================================

pub fn show_interview_setup() -> bool {
    // Load existing data
    if let Some(profile) = load_user_profile() {
        DATA.with(|d| {
            let mut data = d.borrow_mut();
            data.name = profile.name;
            data.target_role = profile.target_role;
            data.resume = profile.resume_text;
        });
    }
    if let Some(keys) = load_api_keys() {
        DATA.with(|d| {
            let mut data = d.borrow_mut();
            data.google_key = keys.google_api_key;
            data.groq_key = keys.groq_api_key;
            data.nvidia_key = keys.nvidia_api_key;
            data.assembly_key = keys.assembly_ai_key;
        });
    }
    
    SETUP_OK.store(false, Ordering::SeqCst);
    SHOW_API.store(false, Ordering::SeqCst);
    
    unsafe {
        init_resources();
        
        let class = std::ffi::CString::new("InterviewSetupV2").unwrap();
        let instance = GetModuleHandleA(None).unwrap();
        
        let wc = WNDCLASSA {
            hInstance: instance.into(),
            lpszClassName: PCSTR(class.as_ptr() as *const u8),
            lpfnWndProc: Some(wnd_proc),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH::default(),
            style: CS_HREDRAW | CS_VREDRAW,
            ..Default::default()
        };
        RegisterClassA(&wc);
        
        let w = 500;
        let h = 520;
        let x = (GetSystemMetrics(SM_CXSCREEN) - w) / 2;
        let y = (GetSystemMetrics(SM_CYSCREEN) - h) / 2;
        
        let title = std::ffi::CString::new("Interview Setup").unwrap();
        let hwnd = CreateWindowExA(
            WS_EX_TOPMOST,
            PCSTR(class.as_ptr() as *const u8),
            PCSTR(title.as_ptr() as *const u8),
            WS_POPUP | WS_VISIBLE,
            x, y, w, h,
            None, None, instance, None,
        );
        
        if hwnd.0 == 0 {
            cleanup_resources();
            return false;
        }
        
        let mut msg = MSG::default();
        while GetMessageA(&mut msg, None, 0, 0).into() {
            TranslateMessage(&msg);
            DispatchMessageA(&msg);
        }
        
        cleanup_resources();
    }
    
    SETUP_OK.load(Ordering::SeqCst)
}

// =============================================================================
// WINDOW PROC
// =============================================================================

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            create_controls(hwnd);
            LRESULT(0)
        }
        
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            paint(hwnd, hdc);
            EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        
        WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC => {
            let hdc = HDC(wp.0 as isize);
            SetTextColor(hdc, COLORREF(COLOR_TEXT));
            SetBkColor(hdc, COLORREF(COLOR_INPUT));
            LRESULT(BRUSH_INPUT.lock().unwrap().0 as isize)
        }
        
        WM_COMMAND => {
            let id = (wp.0 & 0xFFFF) as i32;
            match id {
                ID_START => {
                    if save_data(hwnd) {
                        SETUP_OK.store(true, Ordering::SeqCst);
                        let _ = DestroyWindow(hwnd);
                    }
                }
                ID_API_BTN => {
                    let show = !SHOW_API.load(Ordering::SeqCst);
                    SHOW_API.store(show, Ordering::SeqCst);
                    toggle_api(hwnd, show);
                }
                ID_CANCEL => {
                    SETUP_OK.store(false, Ordering::SeqCst);
                    let _ = DestroyWindow(hwnd);
                }
                _ => {}
            }
            LRESULT(0)
        }
        
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        
        WM_CLOSE => {
            SETUP_OK.store(false, Ordering::SeqCst);
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        
        _ => DefWindowProcA(hwnd, msg, wp, lp),
    }
}

// =============================================================================
// UI
// =============================================================================

unsafe fn create_controls(hwnd: HWND) {
    let inst: HINSTANCE = GetModuleHandleA(None).unwrap().into();
    
    let (name, role, resume, gkey, groqkey, nkey, akey) = DATA.with(|d| {
        let data = d.borrow();
        (data.name.clone(), data.target_role.clone(), data.resume.clone(),
         data.google_key.clone(), data.groq_key.clone(), data.nvidia_key.clone(), data.assembly_key.clone())
    });
    
    // Name
    make_edit(hwnd, inst, &name, 40, 100, 420, 32, ID_NAME, ES_AUTOHSCROLL);
    
    // Target role
    make_edit(hwnd, inst, &role, 40, 175, 420, 32, ID_ROLE, ES_AUTOHSCROLL);
    
    // Resume - large text area
    make_edit(hwnd, inst, &resume, 40, 250, 420, 130, ID_RESUME, ES_MULTILINE | ES_AUTOVSCROLL);
    
    // API section (hidden by default)
    make_edit_hidden(hwnd, inst, &gkey, 40, 395, 100, 28, ID_GOOGLE, ES_AUTOHSCROLL | ES_PASSWORD);
    make_edit_hidden(hwnd, inst, &groqkey, 145, 395, 100, 28, ID_GROQ, ES_AUTOHSCROLL | ES_PASSWORD);
    make_edit_hidden(hwnd, inst, &nkey, 250, 395, 100, 28, ID_NVIDIA, ES_AUTOHSCROLL | ES_PASSWORD);
    make_edit_hidden(hwnd, inst, &akey, 355, 395, 100, 28, ID_ASSEMBLY, ES_AUTOHSCROLL | ES_PASSWORD);
    
    // Buttons
    make_btn(hwnd, inst, "Update API Keys", 40, 395, 130, 30, ID_API_BTN, false);
    make_btn(hwnd, inst, "Start Interview", 300, 460, 160, 40, ID_START, true);
    make_btn(hwnd, inst, "Cancel", 40, 465, 80, 32, ID_CANCEL, false);
}

unsafe fn make_edit(hwnd: HWND, inst: HINSTANCE, text: &str, x: i32, y: i32, w: i32, h: i32, id: i32, style: u32) {
    let class = std::ffi::CString::new("EDIT").unwrap();
    let txt = std::ffi::CString::new(text).unwrap();
    let edit = CreateWindowExA(
        WS_EX_CLIENTEDGE,
        PCSTR(class.as_ptr() as *const u8),
        PCSTR(txt.as_ptr() as *const u8),
        WS_VISIBLE | WS_CHILD | WS_TABSTOP | WINDOW_STYLE(style),
        x, y, w, h,
        hwnd, HMENU(id as _), inst, None,
    );
    SendMessageA(edit, WM_SETFONT, WPARAM(FONT_BODY.lock().unwrap().0 as usize), LPARAM(1));
}

unsafe fn make_edit_hidden(hwnd: HWND, inst: HINSTANCE, text: &str, x: i32, y: i32, w: i32, h: i32, id: i32, style: u32) {
    let class = std::ffi::CString::new("EDIT").unwrap();
    let txt = std::ffi::CString::new(text).unwrap();
    let edit = CreateWindowExA(
        WS_EX_CLIENTEDGE,
        PCSTR(class.as_ptr() as *const u8),
        PCSTR(txt.as_ptr() as *const u8),
        WS_CHILD | WS_TABSTOP | WINDOW_STYLE(style), // No WS_VISIBLE
        x, y, w, h,
        hwnd, HMENU(id as _), inst, None,
    );
    SendMessageA(edit, WM_SETFONT, WPARAM(FONT_SMALL.lock().unwrap().0 as usize), LPARAM(1));
}

unsafe fn make_btn(hwnd: HWND, inst: HINSTANCE, text: &str, x: i32, y: i32, w: i32, h: i32, id: i32, primary: bool) {
    let class = std::ffi::CString::new("BUTTON").unwrap();
    let txt = std::ffi::CString::new(text).unwrap();
    let style = if primary { 0x00000001u32 } else { 0u32 };
    let btn = CreateWindowExA(
        WINDOW_EX_STYLE::default(),
        PCSTR(class.as_ptr() as *const u8),
        PCSTR(txt.as_ptr() as *const u8),
        WS_VISIBLE | WS_CHILD | WS_TABSTOP | WINDOW_STYLE(style),
        x, y, w, h,
        hwnd, HMENU(id as _), inst, None,
    );
    let font = if primary { FONT_BODY.lock().unwrap().0 } else { FONT_SMALL.lock().unwrap().0 };
    SendMessageA(btn, WM_SETFONT, WPARAM(font as usize), LPARAM(1));
}

unsafe fn toggle_api(hwnd: HWND, show: bool) {
    let sw = if show { SW_SHOW } else { SW_HIDE };
    ShowWindow(GetDlgItem(hwnd, ID_GOOGLE), sw);
    ShowWindow(GetDlgItem(hwnd, ID_GROQ), sw);
    ShowWindow(GetDlgItem(hwnd, ID_NVIDIA), sw);
    ShowWindow(GetDlgItem(hwnd, ID_ASSEMBLY), sw);
    
    // Move/hide the toggle button
    let btn = GetDlgItem(hwnd, ID_API_BTN);
    if show {
        let _ = SetWindowPos(btn, None, 40, 430, 130, 25, SWP_NOZORDER);
        let txt = std::ffi::CString::new("Hide API Keys").unwrap();
        SetWindowTextA(btn, PCSTR(txt.as_ptr() as *const u8));
    } else {
        let _ = SetWindowPos(btn, None, 40, 395, 130, 30, SWP_NOZORDER);
        let txt = std::ffi::CString::new("Update API Keys").unwrap();
        SetWindowTextA(btn, PCSTR(txt.as_ptr() as *const u8));
    }
    
    let _ = InvalidateRect(hwnd, None, TRUE);
}

unsafe fn paint(hwnd: HWND, hdc: HDC) {
    let mut r = RECT::default();
    let _ = GetClientRect(hwnd, &mut r);
    
    // Background
    FillRect(hdc, &r, *BRUSH_BG.lock().unwrap());
    
    // Header
    let header = RECT { left: 0, top: 0, right: r.right, bottom: 70 };
    FillRect(hdc, &header, *BRUSH_CARD.lock().unwrap());
    
    // Accent line
    let accent = RECT { left: 0, top: 68, right: r.right, bottom: 70 };
    FillRect(hdc, &accent, *BRUSH_ACCENT.lock().unwrap());
    
    SetBkMode(hdc, TRANSPARENT);
    
    // Title
    SelectObject(hdc, *FONT_TITLE.lock().unwrap());
    SetTextColor(hdc, COLORREF(COLOR_TEXT));
    TextOutA(hdc, 40, 15, "Interview Setup".as_bytes());
    
    // Subtitle
    SelectObject(hdc, *FONT_SMALL.lock().unwrap());
    SetTextColor(hdc, COLORREF(COLOR_TEXT_DIM));
    TextOutA(hdc, 40, 45, "Tell AI about yourself for personalized answers".as_bytes());
    
    // Labels
    SelectObject(hdc, *FONT_SMALL.lock().unwrap());
    SetTextColor(hdc, COLORREF(COLOR_TEXT_DIM));
    TextOutA(hdc, 40, 82, "Your Name".as_bytes());
    TextOutA(hdc, 40, 157, "Target Role (what position is this interview for?)".as_bytes());
    TextOutA(hdc, 40, 232, "Your Resume / Background (paste here - AI learns from this)".as_bytes());
    
    // API labels if visible
    if SHOW_API.load(Ordering::SeqCst) {
        TextOutA(hdc, 40, 378, "Google Key".as_bytes());
        TextOutA(hdc, 145, 378, "Groq Key".as_bytes());
        TextOutA(hdc, 250, 378, "NVIDIA Key".as_bytes());
        TextOutA(hdc, 355, 378, "Assembly Key".as_bytes());
    }
}

// =============================================================================
// RESOURCES
// =============================================================================

unsafe fn init_resources() {
    *BRUSH_BG.lock().unwrap() = CreateSolidBrush(COLORREF(COLOR_BG));
    *BRUSH_CARD.lock().unwrap() = CreateSolidBrush(COLORREF(COLOR_CARD));
    *BRUSH_ACCENT.lock().unwrap() = CreateSolidBrush(COLORREF(COLOR_ACCENT));
    *BRUSH_INPUT.lock().unwrap() = CreateSolidBrush(COLORREF(COLOR_INPUT));
    
    let font = PCSTR("Segoe UI\0".as_ptr());
    *FONT_TITLE.lock().unwrap() = CreateFontA(24, 0, 0, 0, 600, 0, 0, 0, 0, 0, 0, 4, 0, font);
    *FONT_BODY.lock().unwrap() = CreateFontA(16, 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 4, 0, font);
    *FONT_SMALL.lock().unwrap() = CreateFontA(13, 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 4, 0, font);
}

unsafe fn cleanup_resources() {
    let _ = DeleteObject(*BRUSH_BG.lock().unwrap());
    let _ = DeleteObject(*BRUSH_CARD.lock().unwrap());
    let _ = DeleteObject(*BRUSH_ACCENT.lock().unwrap());
    let _ = DeleteObject(*BRUSH_INPUT.lock().unwrap());
    let _ = DeleteObject(*FONT_TITLE.lock().unwrap());
    let _ = DeleteObject(*FONT_BODY.lock().unwrap());
    let _ = DeleteObject(*FONT_SMALL.lock().unwrap());
}

// =============================================================================
// SAVE
// =============================================================================

unsafe fn get_text(hwnd: HWND, id: i32) -> String {
    let ctrl = GetDlgItem(hwnd, id);
    if ctrl.0 == 0 { return String::new(); }
    let mut buf = [0u8; 16384];
    let len = GetWindowTextA(ctrl, &mut buf);
    String::from_utf8_lossy(&buf[..len as usize]).trim().to_string()
}

unsafe fn save_data(hwnd: HWND) -> bool {
    let name = get_text(hwnd, ID_NAME);
    let role = get_text(hwnd, ID_ROLE);
    let resume = get_text(hwnd, ID_RESUME);
    
    if name.is_empty() {
        return false;
    }
    
    // Save profile - AI will extract details from resume
    let profile = UserProfile {
        name,
        email: String::new(),
        phone: String::new(),
        current_role: String::new(),
        experience_years: 0,
        skills: vec![],
        technologies: vec![],
        summary: String::new(),
        resume_text: resume,
        target_role: role,
        target_company: String::new(),
        interview_notes: String::new(),
        response_style: ResponseStyle::Balanced,
        include_examples: true,
    };
    
    if save_user_profile(&profile).is_err() {
        return false;
    }
    
    // Save API keys if visible and changed
    if SHOW_API.load(Ordering::SeqCst) {
        let gkey = get_text(hwnd, ID_GOOGLE);
        let groqkey = get_text(hwnd, ID_GROQ);
        let nkey = get_text(hwnd, ID_NVIDIA);
        let akey = get_text(hwnd, ID_ASSEMBLY);
        
        if !(gkey.is_empty() && groqkey.is_empty() && nkey.is_empty()) && !akey.is_empty() {
            let keys = ApiKeys {
                google_api_key: gkey.clone(),
                groq_api_key: groqkey,
                nvidia_api_key: nkey,
                assembly_ai_key: akey,
                screenshot_api_key: gkey, // Same as google key
            };
            let _ = save_api_keys(&keys);
        }
    }
    
    true
}
