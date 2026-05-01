//! License Wizard - One-time Setup
//! 
//! Step 1: License activation
//! Step 2: API keys (Google + AssemblyAI)
//!
//! Clean Premium Dark UI

use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleA,
        System::Memory::*,
        System::DataExchange::*,
        UI::WindowsAndMessaging::*,
    },
};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;

use crate::license::{get_device_id, verify_license, save_license, LicenseState};
use crate::config::{ApiKeys, save_api_keys};

// Colors
const COLOR_BG: u32 = 0x001A1A1A;
const COLOR_CARD: u32 = 0x00262626;
const COLOR_ACCENT: u32 = 0x0000D4AA;
const COLOR_TEXT: u32 = 0x00FFFFFF;
const COLOR_DIM: u32 = 0x00888888;
const COLOR_INPUT: u32 = 0x00333333;

#[derive(Debug, Clone)]
pub enum SetupResult {
    Completed,
    Cancelled,
    Error(String),
}

static DONE: AtomicBool = AtomicBool::new(false);
static PAGE: AtomicU8 = AtomicU8::new(0);

lazy_static::lazy_static! {
    static ref DEVICE_ID: Mutex<String> = Mutex::new(String::new());
    static ref BRUSH_BG: Mutex<HBRUSH> = Mutex::new(HBRUSH::default());
    static ref BRUSH_CARD: Mutex<HBRUSH> = Mutex::new(HBRUSH::default());
    static ref BRUSH_ACCENT: Mutex<HBRUSH> = Mutex::new(HBRUSH::default());
    static ref BRUSH_INPUT: Mutex<HBRUSH> = Mutex::new(HBRUSH::default());
    static ref FONT_TITLE: Mutex<HFONT> = Mutex::new(HFONT::default());
    static ref FONT_BODY: Mutex<HFONT> = Mutex::new(HFONT::default());
    static ref FONT_MONO: Mutex<HFONT> = Mutex::new(HFONT::default());
    static ref FONT_SMALL: Mutex<HFONT> = Mutex::new(HFONT::default());
}

// IDs
const ID_LICENSE: i32 = 10;
const ID_GOOGLE: i32 = 11;
const ID_GROQ: i32 = 13;
const ID_NVIDIA: i32 = 14;
const ID_ASSEMBLY: i32 = 12;
const ID_COPY: i32 = 20;
const ID_NEXT: i32 = 21;
const ID_EXIT: i32 = 22;
const ID_STATUS: i32 = 30;

const ES_AUTOHSCROLL: u32 = 0x0080;
const ES_CENTER: u32 = 0x0001;
const ES_PASSWORD: u32 = 0x0020;

pub fn run_setup_wizard() -> SetupResult {
    *DEVICE_ID.lock().unwrap() = get_device_id();
    DONE.store(false, Ordering::SeqCst);
    PAGE.store(0, Ordering::SeqCst);
    
    unsafe {
        init_res();
        
        let class = std::ffi::CString::new("LicenseWizardV2").unwrap();
        let inst = GetModuleHandleA(None).unwrap();
        
        let wc = WNDCLASSA {
            hInstance: inst.into(),
            lpszClassName: PCSTR(class.as_ptr() as *const u8),
            lpfnWndProc: Some(wnd_proc),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH::default(),
            style: CS_HREDRAW | CS_VREDRAW,
            ..Default::default()
        };
        RegisterClassA(&wc);
        
        let w = 480;
        let h = 400;
        let x = (GetSystemMetrics(SM_CXSCREEN) - w) / 2;
        let y = (GetSystemMetrics(SM_CYSCREEN) - h) / 2;
        
        let title = std::ffi::CString::new("Setup").unwrap();
        let hwnd = CreateWindowExA(
            WS_EX_TOPMOST,
            PCSTR(class.as_ptr() as *const u8),
            PCSTR(title.as_ptr() as *const u8),
            WS_POPUP | WS_VISIBLE,
            x, y, w, h,
            None, None, inst, None,
        );
        
        if hwnd.0 == 0 {
            cleanup_res();
            return SetupResult::Error("Window failed".into());
        }
        
        let mut msg = MSG::default();
        while GetMessageA(&mut msg, None, 0, 0).into() {
            TranslateMessage(&msg);
            DispatchMessageA(&msg);
        }
        
        cleanup_res();
    }
    
    if DONE.load(Ordering::SeqCst) { SetupResult::Completed } else { SetupResult::Cancelled }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            create_page(hwnd, 0);
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
                ID_COPY => copy_id(hwnd),
                ID_NEXT => next_action(hwnd),
                ID_EXIT => {
                    DONE.store(false, Ordering::SeqCst);
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
            DONE.store(false, Ordering::SeqCst);
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        _ => DefWindowProcA(hwnd, msg, wp, lp),
    }
}

unsafe fn create_page(hwnd: HWND, page: u8) {
    // Clear existing
    let mut child = GetWindow(hwnd, GW_CHILD);
    while child.0 != 0 {
        let next = GetWindow(child, GW_HWNDNEXT);
        let _ = DestroyWindow(child);
        child = next;
    }
    
    let inst: HINSTANCE = GetModuleHandleA(None).unwrap().into();
    
    match page {
        0 => {
            // License page
            mk_edit(hwnd, inst, "", 50, 210, 380, 36, ID_LICENSE, ES_AUTOHSCROLL | ES_CENTER);
            mk_btn(hwnd, inst, "Copy Device ID", 50, 155, 140, 32, ID_COPY, false);
            mk_btn(hwnd, inst, "Activate", 320, 270, 110, 36, ID_NEXT, true);
            mk_btn(hwnd, inst, "Exit", 50, 270, 80, 36, ID_EXIT, false);
            mk_label(hwnd, inst, "", 50, 320, 380, 20, ID_STATUS);
        }
        1 => {
            // API keys page
            mk_edit(hwnd, inst, "", 50, 90, 380, 28, ID_GOOGLE, ES_AUTOHSCROLL | ES_PASSWORD);
            mk_edit(hwnd, inst, "", 50, 150, 380, 28, ID_GROQ, ES_AUTOHSCROLL | ES_PASSWORD);
            mk_edit(hwnd, inst, "", 50, 210, 380, 28, ID_NVIDIA, ES_AUTOHSCROLL | ES_PASSWORD);
            mk_edit(hwnd, inst, "", 50, 270, 380, 28, ID_ASSEMBLY, ES_AUTOHSCROLL | ES_PASSWORD);
            mk_btn(hwnd, inst, "Finish Setup", 290, 320, 140, 40, ID_NEXT, true);
            mk_btn(hwnd, inst, "Exit", 50, 320, 80, 32, ID_EXIT, false);
            mk_label(hwnd, inst, "", 50, 370, 380, 20, ID_STATUS);
        }
        _ => {}
    }
    
    let _ = InvalidateRect(hwnd, None, TRUE);
}

unsafe fn mk_edit(hwnd: HWND, inst: HINSTANCE, txt: &str, x: i32, y: i32, w: i32, h: i32, id: i32, style: u32) {
    let c = std::ffi::CString::new("EDIT").unwrap();
    let t = std::ffi::CString::new(txt).unwrap();
    let e = CreateWindowExA(WS_EX_CLIENTEDGE, PCSTR(c.as_ptr() as *const u8), PCSTR(t.as_ptr() as *const u8),
        WS_VISIBLE | WS_CHILD | WS_TABSTOP | WINDOW_STYLE(style), x, y, w, h, hwnd, HMENU(id as _), inst, None);
    let font = if style & ES_CENTER != 0 { *FONT_MONO.lock().unwrap() } else { *FONT_BODY.lock().unwrap() };
    SendMessageA(e, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
}

unsafe fn mk_btn(hwnd: HWND, inst: HINSTANCE, txt: &str, x: i32, y: i32, w: i32, h: i32, id: i32, primary: bool) {
    let c = std::ffi::CString::new("BUTTON").unwrap();
    let t = std::ffi::CString::new(txt).unwrap();
    let s = if primary { 0x00000001u32 } else { 0u32 };
    let b = CreateWindowExA(WINDOW_EX_STYLE::default(), PCSTR(c.as_ptr() as *const u8), PCSTR(t.as_ptr() as *const u8),
        WS_VISIBLE | WS_CHILD | WS_TABSTOP | WINDOW_STYLE(s), x, y, w, h, hwnd, HMENU(id as _), inst, None);
    let font = if primary { *FONT_BODY.lock().unwrap() } else { *FONT_SMALL.lock().unwrap() };
    SendMessageA(b, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
}

unsafe fn mk_label(hwnd: HWND, inst: HINSTANCE, txt: &str, x: i32, y: i32, w: i32, h: i32, id: i32) {
    let c = std::ffi::CString::new("STATIC").unwrap();
    let t = std::ffi::CString::new(txt).unwrap();
    let l = CreateWindowExA(WINDOW_EX_STYLE::default(), PCSTR(c.as_ptr() as *const u8), PCSTR(t.as_ptr() as *const u8),
        WS_VISIBLE | WS_CHILD, x, y, w, h, hwnd, HMENU(id as _), inst, None);
    SendMessageA(l, WM_SETFONT, WPARAM(FONT_SMALL.lock().unwrap().0 as usize), LPARAM(1));
}

unsafe fn paint(hwnd: HWND, hdc: HDC) {
    let mut r = RECT::default();
    let _ = GetClientRect(hwnd, &mut r);
    let page = PAGE.load(Ordering::SeqCst);
    let device_id = DEVICE_ID.lock().unwrap().clone();
    
    FillRect(hdc, &r, *BRUSH_BG.lock().unwrap());
    
    let header = RECT { left: 0, top: 0, right: r.right, bottom: 65 };
    FillRect(hdc, &header, *BRUSH_CARD.lock().unwrap());
    
    let accent = RECT { left: 0, top: 63, right: r.right, bottom: 65 };
    FillRect(hdc, &accent, *BRUSH_ACCENT.lock().unwrap());
    
    SetBkMode(hdc, TRANSPARENT);
    SelectObject(hdc, *FONT_TITLE.lock().unwrap());
    SetTextColor(hdc, COLORREF(COLOR_TEXT));
    
    let (title, sub) = match page {
        0 => ("Activate License", "Enter the license key from your purchase"),
        1 => ("API Keys", "Enter your API keys to enable features"),
        _ => ("", ""),
    };
    TextOutA(hdc, 50, 15, title.as_bytes());
    
    SelectObject(hdc, *FONT_SMALL.lock().unwrap());
    SetTextColor(hdc, COLORREF(COLOR_DIM));
    TextOutA(hdc, 50, 42, sub.as_bytes());
    
    // Page content
    match page {
        0 => {
            SelectObject(hdc, *FONT_SMALL.lock().unwrap());
            SetTextColor(hdc, COLORREF(COLOR_DIM));
            TextOutA(hdc, 50, 85, "Your Device ID:".as_bytes());
            
            SelectObject(hdc, *FONT_MONO.lock().unwrap());
            SetTextColor(hdc, COLORREF(COLOR_ACCENT));
            TextOutA(hdc, 50, 110, device_id.as_bytes());
            
            SelectObject(hdc, *FONT_SMALL.lock().unwrap());
            SetTextColor(hdc, COLORREF(COLOR_DIM));
            TextOutA(hdc, 50, 190, "License Key:".as_bytes());
            
            // Step indicator
            TextOutA(hdc, 380, 360, "Step 1 of 2".as_bytes());
        }
        1 => {
            SelectObject(hdc, *FONT_SMALL.lock().unwrap());
            SetTextColor(hdc, COLORREF(COLOR_DIM));
            TextOutA(hdc, 50, 70, "Google API Key (optional):".as_bytes());
            
            TextOutA(hdc, 50, 130, "Groq API Key (Fastest - optional):".as_bytes());

            TextOutA(hdc, 50, 190, "NVIDIA API Key (Mistral - optional):".as_bytes());

            TextOutA(hdc, 50, 250, "AssemblyAI Key (for voice transcription):".as_bytes());
            
            TextOutA(hdc, 380, 360, "Step 2 of 2".as_bytes());
        }
        _ => {}
    }
}

unsafe fn init_res() {
    *BRUSH_BG.lock().unwrap() = CreateSolidBrush(COLORREF(COLOR_BG));
    *BRUSH_CARD.lock().unwrap() = CreateSolidBrush(COLORREF(COLOR_CARD));
    *BRUSH_ACCENT.lock().unwrap() = CreateSolidBrush(COLORREF(COLOR_ACCENT));
    *BRUSH_INPUT.lock().unwrap() = CreateSolidBrush(COLORREF(COLOR_INPUT));
    
    let f = PCSTR("Segoe UI\0".as_ptr());
    let m = PCSTR("Consolas\0".as_ptr());
    *FONT_TITLE.lock().unwrap() = CreateFontA(22, 0, 0, 0, 600, 0, 0, 0, 0, 0, 0, 4, 0, f);
    *FONT_BODY.lock().unwrap() = CreateFontA(15, 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 4, 0, f);
    *FONT_MONO.lock().unwrap() = CreateFontA(16, 0, 0, 0, 500, 0, 0, 0, 0, 0, 0, 4, 0, m);
    *FONT_SMALL.lock().unwrap() = CreateFontA(12, 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 4, 0, f);
}

unsafe fn cleanup_res() {
    let _ = DeleteObject(*BRUSH_BG.lock().unwrap());
    let _ = DeleteObject(*BRUSH_CARD.lock().unwrap());
    let _ = DeleteObject(*BRUSH_ACCENT.lock().unwrap());
    let _ = DeleteObject(*BRUSH_INPUT.lock().unwrap());
    let _ = DeleteObject(*FONT_TITLE.lock().unwrap());
    let _ = DeleteObject(*FONT_BODY.lock().unwrap());
    let _ = DeleteObject(*FONT_MONO.lock().unwrap());
    let _ = DeleteObject(*FONT_SMALL.lock().unwrap());
}

unsafe fn get_txt(hwnd: HWND, id: i32) -> String {
    let e = GetDlgItem(hwnd, id);
    if e.0 == 0 { return String::new(); }
    let mut buf = [0u8; 512];
    let len = GetWindowTextA(e, &mut buf);
    String::from_utf8_lossy(&buf[..len as usize]).trim().to_string()
}

unsafe fn set_status(hwnd: HWND, txt: &str) {
    let s = GetDlgItem(hwnd, ID_STATUS);
    if s.0 != 0 {
        let t = std::ffi::CString::new(txt).unwrap_or_default();
        SetWindowTextA(s, PCSTR(t.as_ptr() as *const u8));
    }
}

unsafe fn copy_id(hwnd: HWND) {
    let id = DEVICE_ID.lock().unwrap().clone();
    if OpenClipboard(hwnd).is_ok() {
        let _ = EmptyClipboard();
        let len = id.len() + 1;
        if let Ok(mem) = GlobalAlloc(GMEM_MOVEABLE, len) {
            let ptr = GlobalLock(mem) as *mut u8;
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(id.as_ptr(), ptr, id.len());
                *ptr.add(id.len()) = 0;
                let _ = GlobalUnlock(mem);
                let _ = SetClipboardData(1, HANDLE(mem.0 as isize));
            }
        }
        let _ = CloseClipboard();
        set_status(hwnd, "Copied! Send to seller to get license.");
    }
}

unsafe fn next_action(hwnd: HWND) {
    let page = PAGE.load(Ordering::SeqCst);
    
    match page {
        0 => {
            let key = get_txt(hwnd, ID_LICENSE);
            if key.is_empty() {
                set_status(hwnd, "Enter license key");
                return;
            }
            match verify_license(&key) {
                LicenseState::Valid => {
                    if save_license(&key).is_ok() {
                        PAGE.store(1, Ordering::SeqCst);
                        create_page(hwnd, 1);
                    } else {
                        set_status(hwnd, "Failed to save");
                    }
                }
                _ => set_status(hwnd, "Invalid license key"),
            }
        }
        1 => {
            let gkey = get_txt(hwnd, ID_GOOGLE);
            let groqkey = get_txt(hwnd, ID_GROQ);
            let nkey = get_txt(hwnd, ID_NVIDIA);
            let akey = get_txt(hwnd, ID_ASSEMBLY);
            
            if (gkey.is_empty() && groqkey.is_empty() && nkey.is_empty()) || akey.is_empty() {
                set_status(hwnd, "Assembly + one AI key required");
                return;
            }
            
            let keys = ApiKeys {
                google_api_key: gkey.clone(),
                groq_api_key: groqkey,
                nvidia_api_key: nkey,
                assembly_ai_key: akey,
                screenshot_api_key: gkey,
            };
            
            if save_api_keys(&keys).is_ok() {
                DONE.store(true, Ordering::SeqCst);
                let _ = DestroyWindow(hwnd);
            } else {
                set_status(hwnd, "Failed to save");
            }
        }
        _ => {}
    }
}
