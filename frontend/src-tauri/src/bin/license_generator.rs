//! License Key Generator
//! 
//! YOUR TOOL to generate license keys for customers.
//! 
//! USAGE:
//!   license_generator.exe <device_id>
//! 
//! EXAMPLE:
//!   license_generator.exe A7F3-B2C1-D9E4-F5A6
//!   Output: SM-1234-5678-ABCD-EF90
//!
//! Keep this tool SECRET! Only you should have access to it.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// =============================================================================
// 🔐 SECRET KEY - MUST MATCH THE ONE IN verify.rs
// =============================================================================
const LICENSE_SECRET: &str = "SM_INTERVIEW_HELPER_2024_PREMIUM_LICENSE_KEY";
const LICENSE_VERSION: u32 = 1;

fn main() {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║          🔑 INTERVIEW HELPER - LICENSE GENERATOR 🔑          ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
    
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        println!("USAGE: {} <device_id>", args[0]);
        println!();
        println!("EXAMPLE:");
        println!("  {} A7F3-B2C1-D9E4-F5A6", args[0]);
        println!();
        println!("The device_id is shown to the customer in the app.");
        return;
    }
    
    let device_id = args[1].trim().to_uppercase();
    
    // Validate device ID format
    if !is_valid_device_id(&device_id) {
        println!("❌ ERROR: Invalid device ID format!");
        println!("   Expected format: XXXX-XXXX-XXXX-XXXX");
        println!("   Got: {}", device_id);
        return;
    }
    
    let license = generate_license(&device_id);
    
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│  Device ID:   {}                           │", device_id);
    println!("├─────────────────────────────────────────────────────────────┤");
    println!("│  LICENSE KEY: {}                      │", license);
    println!("└─────────────────────────────────────────────────────────────┘");
    println!();
    println!("✅ Send this license key to the customer!");
    println!();
    
    // Copy to clipboard on Windows
    #[cfg(windows)]
    {
        if let Ok(mut child) = std::process::Command::new("cmd")
            .args(["/C", &format!("echo {}| clip", license)])
            .spawn()
        {
            let _ = child.wait();
            println!("📋 License key copied to clipboard!");
        }
    }
}

fn generate_license(device_id: &str) -> String {
    // Combine device ID with secret
    let combined = format!("{}:{}:{}", device_id, LICENSE_SECRET, LICENSE_VERSION);
    
    // Multi-round hashing for security
    let mut hash = hash_string(&combined);
    for _ in 0..1000 {
        hash = hash_string(&format!("{}:{}", hash, LICENSE_SECRET));
    }
    
    // Format as license key: SM-XXXX-XXXX-XXXX-XXXX
    let hex = format!("{:016X}", hash);
    format!(
        "SM-{}-{}-{}-{}",
        &hex[0..4],
        &hex[4..8],
        &hex[8..12],
        &hex[12..16]
    )
}

fn hash_string(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

fn is_valid_device_id(id: &str) -> bool {
    // Format: XXXX-XXXX-XXXX-XXXX
    if id.len() != 19 {
        return false;
    }
    
    let parts: Vec<&str> = id.split('-').collect();
    if parts.len() != 4 {
        return false;
    }
    
    for part in parts {
        if part.len() != 4 {
            return false;
        }
        if !part.chars().all(|c| c.is_ascii_hexdigit()) {
            return false;
        }
    }
    
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_license_generation() {
        let device_id = "A7F3-B2C1-D9E4-F5A6";
        let license1 = generate_license(device_id);
        let license2 = generate_license(device_id);
        assert_eq!(license1, license2);
        assert!(license1.starts_with("SM-"));
    }
    
    #[test]
    fn test_different_devices_different_licenses() {
        let license1 = generate_license("A7F3-B2C1-D9E4-F5A6");
        let license2 = generate_license("AAAA-BBBB-CCCC-DDDD");
        assert_ne!(license1, license2);
    }
    
    #[test]
    fn test_valid_device_id() {
        assert!(is_valid_device_id("A7F3-B2C1-D9E4-F5A6"));
        assert!(is_valid_device_id("1234-5678-9ABC-DEF0"));
        assert!(!is_valid_device_id("A7F3B2C1D9E4F5A6"));
        assert!(!is_valid_device_id("A7F3-B2C1-D9E4"));
        assert!(!is_valid_device_id("XXXX-YYYY-ZZZZ-WWWW"));
    }
}
