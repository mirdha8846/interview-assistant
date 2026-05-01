use std::path::{Path, PathBuf};
use std::fs::{File, OpenOptions};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::env;
use windows::core::Result;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileA, GENERIC_READ, GENERIC_WRITE, OPEN_EXISTING, FILE_SHARE_NONE,
    FILE_ATTRIBUTE_NORMAL, LockFileEx, UnlockFileEx, LOCKFILE_EXCLUSIVE_LOCK,
    LOCKFILE_FAIL_IMMEDIATELY
};
use std::ffi::CString;

static mut FILE_LOCK_HANDLE: Option<HANDLE> = None;
static INTEGRITY_RUNNING: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

pub fn lock_executable_file() -> Result<()> {
    unsafe {
        let current_exe = env::current_exe().unwrap();
        let path_str = CString::new(current_exe.to_str().unwrap()).unwrap();
        
        let handle = CreateFileA(
            windows::core::PCSTR(path_str.as_ptr() as *const u8),
            GENERIC_READ,
            FILE_SHARE_NONE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None
        )?;

        // Lock the entire file
        let lock_result = LockFileEx(
            handle,
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            0xFFFFFFFF,
            0xFFFFFFFF,
            None
        );

        if lock_result.is_ok() {
            FILE_LOCK_HANDLE = Some(handle);
            println!("Executable file locked successfully");
            Ok(())
        } else {
            let _ = CloseHandle(handle);
            Err(windows::core::Error::from_win32())
        }
    }
}

pub fn monitor_file_integrity() {
    let running = Arc::clone(&INTEGRITY_RUNNING);
    {
        let mut guard = running.lock().unwrap();
        if *guard {
            return; // Already running
        }
        *guard = true;
    }

    thread::spawn(move || {
        let current_exe = env::current_exe().unwrap();
        let original_metadata = std::fs::metadata(&current_exe).ok();
        
        loop {
            thread::sleep(Duration::from_secs(5));
            
            // Check if running flag is still true
            {
                let guard = running.lock().unwrap();
                if !*guard {
                    break;
                }
            }

            // Check file integrity
            if let Ok(current_metadata) = std::fs::metadata(&current_exe) {
                if let Some(ref original) = original_metadata {
                    // Compare file size and modification time
                    if current_metadata.len() != original.len() 
                        || current_metadata.modified().unwrap() != original.modified().unwrap() {
                        println!("WARNING: Executable file has been modified!");
                        
                        // Attempt to restore from backup
                        if let Err(e) = restore_from_backup() {
                            println!("Failed to restore from backup: {:?}", e);
                            std::process::exit(1);
                        }
                    }
                }
            } else {
                println!("WARNING: Executable file no longer exists!");
                
                // Attempt to restore from backup
                if let Err(e) = restore_from_backup() {
                    println!("Failed to restore executable: {:?}", e);
                    std::process::exit(1);
                }
            }
        }
        
        println!("File integrity monitoring stopped");
    });
}

pub fn create_backup_copy(dest: &Path) -> std::io::Result<()> {
    let current_exe = env::current_exe()?;
    std::fs::copy(&current_exe, dest)?;
    
    // Set hidden attribute
    let path_str = CString::new(dest.to_str().unwrap()).unwrap();
    unsafe {
        let _ = windows::Win32::Storage::FileSystem::SetFileAttributesA(
            windows::core::PCSTR(path_str.as_ptr() as *const u8),
            windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_HIDDEN
        );
    }
    
    println!("Backup copy created: {:?}", dest);
    Ok(())
}

pub fn spawn_watchdog_process() -> Result<()> {
    let current_exe = env::current_exe().unwrap();
    let current_pid = std::process::id();
    
    // Create a simple watchdog that monitors our process
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(10));
            
            // Check if main process is still running
            let output = std::process::Command::new("tasklist")
                .args(["/FI", &format!("PID eq {}", current_pid)])
                .output();
                
            if let Ok(output) = output {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if !stdout.contains(&current_pid.to_string()) {
                    println!("Main process terminated, restarting...");
                    
                    // Restart the process
                    let _ = std::process::Command::new(&current_exe)
                        .spawn();
                    break;
                }
            }
        }
    });
    
    println!("Watchdog process spawned");
    Ok(())
}

fn restore_from_backup() -> std::io::Result<()> {
    let current_exe = env::current_exe()?;
    let backup_path = get_backup_path();
    
    if backup_path.exists() {
        std::fs::copy(&backup_path, &current_exe)?;
        println!("Executable restored from backup");
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Backup file not found"
        ))
    }
}

fn get_backup_path() -> PathBuf {
    let current_exe = env::current_exe().unwrap();
    let parent = current_exe.parent().unwrap();
    parent.join(".sysbackup")
}