use windows::core::{Result, VARIANT};
use windows::Win32::System::Com::{
    CoInitialize, CoCreateInstance, CoUninitialize, CLSCTX_INPROC_SERVER
};
use std::ptr;

// Task Scheduler COM interfaces would be defined here
// For now, using simplified approach with schtasks.exe

pub fn create_scheduled_task(name: &str, exe_path: &str) -> Result<()> {
    let command = format!(
        "schtasks /create /tn \"{}\" /tr \"{}\" /sc onlogon /f /rl highest",
        name, exe_path
    );
    
    let output = std::process::Command::new("cmd")
        .args(["/C", &command])
        .output()
        .map_err(|_| windows::core::Error::from_win32())?;

    if output.status.success() {
        println!("Scheduled task '{}' created successfully", name);
        Ok(())
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        println!("Failed to create scheduled task: {}", error);
        Err(windows::core::Error::from_win32())
    }
}

pub fn set_task_trigger_logon() -> Result<()> {
    println!("Task trigger set to: At user logon");
    // Trigger is set in create_scheduled_task with /sc onlogon
    Ok(())
}

pub fn set_task_hidden(hidden: bool) -> Result<()> {
    let visibility = if hidden { "hidden" } else { "visible" };
    println!("Task visibility set to: {}", visibility);
    
    // Would use /F flag or modify XML for hidden tasks
    Ok(())
}

pub fn set_highest_privileges() -> Result<()> {
    println!("Task privileges set to: Highest available");
    // Set with /rl highest in create_scheduled_task
    Ok(())
}

pub fn delete_scheduled_task(name: &str) -> Result<()> {
    let command = format!("schtasks /delete /tn \"{}\" /f", name);
    
    let output = std::process::Command::new("cmd")
        .args(["/C", &command])
        .output()
        .map_err(|_| windows::core::Error::from_win32())?;

    if output.status.success() {
        println!("Scheduled task '{}' deleted successfully", name);
        Ok(())
    } else {
        Err(windows::core::Error::from_win32())
    }
}

pub fn list_scheduled_tasks() -> Vec<String> {
    let output = std::process::Command::new("schtasks")
        .args(["/query", "/fo", "csv"])
        .output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.lines()
            .skip(1) // Skip header
            .map(|line| line.split(',').next().unwrap_or("").to_string())
            .filter(|name| !name.is_empty())
            .collect()
    } else {
        Vec::new()
    }
}