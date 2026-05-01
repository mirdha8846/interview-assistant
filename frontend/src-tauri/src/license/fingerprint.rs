//! Hardware Fingerprinting
//! 
//! Generates a unique Device ID from hardware components:
//! - CPU ID
//! - Disk Serial Number
//! - BIOS UUID
//! - MAC Address

use std::process::Command;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Get unique hardware-based Device ID
/// Format: XXXX-XXXX-XXXX-XXXX
pub fn get_device_id() -> String {
    let raw_id = collect_hardware_info();
    let hash = hash_string(&raw_id);
    format_device_id(hash)
}

/// Collect hardware information
fn collect_hardware_info() -> String {
    let mut info = String::new();
    
    // CPU ID
    if let Some(cpu) = get_cpu_id() {
        info.push_str(&cpu);
    }
    
    // Disk Serial
    if let Some(disk) = get_disk_serial() {
        info.push_str(&disk);
    }
    
    // BIOS UUID
    if let Some(bios) = get_bios_uuid() {
        info.push_str(&bios);
    }
    
    // Machine GUID (Windows)
    if let Some(guid) = get_machine_guid() {
        info.push_str(&guid);
    }
    
    // Fallback if nothing collected
    if info.is_empty() {
        info = get_fallback_id();
    }
    
    info
}

/// Get CPU ID via WMIC
fn get_cpu_id() -> Option<String> {
    let output = Command::new("wmic")
        .args(["cpu", "get", "processorid"])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output()
        .ok()?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .nth(1)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Get primary disk serial number
fn get_disk_serial() -> Option<String> {
    let output = Command::new("wmic")
        .args(["diskdrive", "get", "serialnumber"])
        .creation_flags(0x08000000)
        .output()
        .ok()?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .nth(1)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Get BIOS UUID
fn get_bios_uuid() -> Option<String> {
    let output = Command::new("wmic")
        .args(["csproduct", "get", "uuid"])
        .creation_flags(0x08000000)
        .output()
        .ok()?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .nth(1)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF")
}

/// Get Windows Machine GUID from registry
fn get_machine_guid() -> Option<String> {
    let output = Command::new("reg")
        .args([
            "query",
            "HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Cryptography",
            "/v",
            "MachineGuid"
        ])
        .creation_flags(0x08000000)
        .output()
        .ok()?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.contains("MachineGuid") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                return Some(parts[2].to_string());
            }
        }
    }
    None
}

/// Fallback ID using computer name and username
fn get_fallback_id() -> String {
    let computer = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "UNKNOWN".to_string());
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "USER".to_string());
    format!("{}_{}_FALLBACK", computer, user)
}

/// Hash a string to u64
fn hash_string(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// Format hash as XXXX-XXXX-XXXX-XXXX
fn format_device_id(hash: u64) -> String {
    let hex = format!("{:016X}", hash);
    format!(
        "{}-{}-{}-{}",
        &hex[0..4],
        &hex[4..8],
        &hex[8..12],
        &hex[12..16]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_device_id_format() {
        let id = get_device_id();
        assert_eq!(id.len(), 19); // XXXX-XXXX-XXXX-XXXX
        assert_eq!(id.chars().filter(|c| *c == '-').count(), 3);
    }
    
    #[test]
    fn test_device_id_consistent() {
        let id1 = get_device_id();
        let id2 = get_device_id();
        assert_eq!(id1, id2); // Should be same on same machine
    }
}
