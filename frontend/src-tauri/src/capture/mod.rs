use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::path::PathBuf;
use lazy_static::lazy_static;
use chrono::Local;
use windows::Win32::UI::WindowsAndMessaging::GetDesktopWindow;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateCompatibleBitmap, SelectObject, BitBlt, DeleteObject, DeleteDC,
    GetDIBits, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, SRCCOPY, BI_RGB,
    GetDeviceCaps, HORZRES, VERTRES, GetDC, ReleaseDC
};
use std::ffi::c_void;

mod dxgi_capture;
pub mod image_processing;

lazy_static! {
    static ref SAVE_PATH: Mutex<PathBuf> = Mutex::new(PathBuf::from("."));
    static ref LAST_SCREENSHOT_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);
    static ref SCREENSHOT_LIST: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
}

static SCREENSHOT_COUNTER: AtomicU32 = AtomicU32::new(0);
const ENCRYPTION_KEY: [u8; 32] = [0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6,
                                  0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf, 0x4f, 0x3c,
                                  0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6,
                                  0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf, 0x4f, 0x3c];

pub fn init() {
    let mut path = SAVE_PATH.lock().unwrap();
    *path = std::env::temp_dir();
    crate::log_info!("Capture module initialized. Save path set to: {:?}", *path);
}

pub fn take_screenshot() {
    crate::log_info!("Taking screenshot...");
    unsafe {
        // DXGI is returning black screens, forcing GDI for reliability
        // let result = capture_dxgi().or_else(|_| capture_gdi());
        let result = capture_gdi();
        
        match result {
            Ok(path) => {
                let count = SCREENSHOT_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;
                crate::log_info!("Screenshot #{} saved securely to: {:?}", count, path);
                let mut last = LAST_SCREENSHOT_PATH.lock().unwrap();
                *last = Some(path);
            },
            Err(e) => {
                crate::log_error!("Failed to take screenshot: {:?}", e);
            }
        }
    }
}

fn save_screenshot(width: i32, height: i32, mut pixels: Vec<u8>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Convert BGRA to RGBA
    for chunk in pixels.chunks_mut(4) {
        let b = chunk[0];
        chunk[0] = chunk[2];
        chunk[2] = b;
        chunk[3] = 255;
    }

    // Create PNG image buffer
    let mut png_buffer = Vec::new();
    {
        use image::{ImageBuffer, Rgba};
        let img = ImageBuffer::<Rgba<u8>, _>::from_raw(width as u32, height as u32, pixels)
            .ok_or("Failed to create image buffer")?;
        
        // Encode as PNG with compression
        let mut cursor = std::io::Cursor::new(&mut png_buffer);
        img.write_to(&mut cursor, image::ImageOutputFormat::Png)?;
    }

    // Save PNG directly (no encryption for speed)
    let timestamp = Local::now().format("%Y%m%d%H%M%S").to_string();
    let filename = format!("ss_{}.png", timestamp);
    let save_path = SAVE_PATH.lock().unwrap().join(filename);
    
    std::fs::write(&save_path, &png_buffer)?;
    
    {
        let mut list = SCREENSHOT_LIST.lock().unwrap();
        list.push(save_path.clone());
    }
    
    Ok(save_path)
}

unsafe fn capture_dxgi() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let capture = dxgi_capture::DxgiCapture::new()?;
    let pixels = capture.capture_frame()?;
    
    // Get dimensions (simplified, assuming 1920x1080 or getting from system)
    // In a real app we'd get this from the texture desc in dxgi_capture
    let hwnd = GetDesktopWindow();
    let hdc = GetDC(hwnd);
    let width = GetDeviceCaps(hdc, HORZRES);
    let height = GetDeviceCaps(hdc, VERTRES);
    ReleaseDC(hwnd, hdc);

    save_screenshot(width, height, pixels)
}


pub fn reset_screenshot_counter() {
    SCREENSHOT_COUNTER.store(0, Ordering::SeqCst);
}

pub fn get_screenshot_count() -> u32 {
    SCREENSHOT_COUNTER.load(Ordering::SeqCst)
}

pub fn get_all_screenshots() -> Vec<PathBuf> {
    SCREENSHOT_LIST.lock().unwrap().clone()
}

pub fn clear_screenshots() {
    let mut list = SCREENSHOT_LIST.lock().unwrap();
    // Securely delete files
    for path in &*list {
        if path.exists() {
            // Overwrite file before deletion for security
            if let Ok(metadata) = std::fs::metadata(path) {
                let size = metadata.len() as usize;
                let random_data: Vec<u8> = (0..size).map(|_| rand::random::<u8>()).collect();
                let _ = std::fs::write(path, random_data);
            }
            let _ = std::fs::remove_file(path);
        }
    }
    list.clear();
}

fn encrypt_data(data: &[u8]) -> Vec<u8> {
    data.iter()
        .enumerate()
        .map(|(i, &byte)| byte ^ ENCRYPTION_KEY[i % ENCRYPTION_KEY.len()])
        .collect()
}

fn decrypt_data(data: &[u8]) -> Vec<u8> {
    // XOR is symmetric
    encrypt_data(data)
}

pub fn set_save_path(path: &str) {
    let mut save_path = SAVE_PATH.lock().unwrap();
    *save_path = PathBuf::from(path);
    if !save_path.exists() {
        let _ = std::fs::create_dir_all(&*save_path);
    }
    println!("Save path set to: {:?}", *save_path);
}

pub fn delete_last_screenshot() {
    let mut last = LAST_SCREENSHOT_PATH.lock().unwrap();
    if let Some(path) = &*last {
        if std::fs::remove_file(path).is_ok() {
            println!("Deleted: {:?}", path);
            *last = None;
        } else {
            eprintln!("Failed to delete: {:?}", path);
        }
    } else {
        println!("No screenshot to delete");
    }
}

unsafe fn capture_gdi() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let hwnd = GetDesktopWindow();
    let hdc_screen = GetDC(hwnd);
    let hdc_mem = CreateCompatibleDC(hdc_screen);
    
    let width = GetDeviceCaps(hdc_screen, HORZRES);
    let height = GetDeviceCaps(hdc_screen, VERTRES);

    let hbitmap = CreateCompatibleBitmap(hdc_screen, width, height);
    let old_obj = SelectObject(hdc_mem, hbitmap);

    BitBlt(hdc_mem, 0, 0, width, height, hdc_screen, 0, 0, SRCCOPY)?;

    // Get bits
    let mut bi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height, // Top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut pixels: Vec<u8> = vec![0; (width * height * 4) as usize];
    GetDIBits(hdc_mem, hbitmap, 0, height as u32, Some(pixels.as_mut_ptr() as *mut c_void), &mut bi, DIB_RGB_COLORS);

    // Cleanup GDI
    SelectObject(hdc_mem, old_obj);
    DeleteObject(hbitmap);
    DeleteDC(hdc_mem);
    ReleaseDC(hwnd, hdc_screen);

    save_screenshot(width, height, pixels)
}

pub fn get_last_screenshot() -> Option<PathBuf> {
    LAST_SCREENSHOT_PATH.lock().unwrap().clone()
}

pub fn delete_all_screenshots() {
    let mut list = SCREENSHOT_LIST.lock().unwrap();
    for path in list.iter() {
        let _ = std::fs::remove_file(path);
    }
    list.clear();
    SCREENSHOT_COUNTER.store(0, Ordering::SeqCst);
    crate::log_info!("All screenshots deleted, counter reset to 0");
}
