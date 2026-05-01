//! License Verification
//! 
//! Verifies license keys against device ID using HMAC-like signature

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::fs;
use serde::{Deserialize, Serialize};

use super::fingerprint::get_device_id;

// =============================================================================
// 🔐 SECRET KEY - This is embedded in app (obfuscated in release)
// =============================================================================
// In production, this should be more obfuscated/protected
const LICENSE_SECRET: &str = "SM_INTERVIEW_HELPER_2024_PREMIUM_LICENSE_KEY";
const LICENSE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub enum LicenseState {
    NotActivated,
    Valid,
    Invalid,
    Expired,
}

#[derive(Serialize, Deserialize, Debug)]
struct LicenseData {
    license_key: String,
    device_id: String,
    activated_at: String,
    version: u32,
}

/// Check if app is licensed
pub fn is_licensed() -> bool {
    match load_saved_license() {
        Some(data) => {
            let current_device = get_device_id();
            if data.device_id != current_device {
                return false; // Device changed
            }
            verify_license(&data.license_key) == LicenseState::Valid
        }
        None => false,
    }
}

/// Verify a license key against current device
pub fn verify_license(license_key: &str) -> LicenseState {
    let device_id = get_device_id();
    let expected = generate_license_internal(&device_id);
    
    // Clean up input
    let clean_key = license_key.trim().to_uppercase().replace(" ", "");
    let clean_expected = expected.trim().to_uppercase().replace(" ", "");
    
    if clean_key == clean_expected {
        LicenseState::Valid
    } else {
        LicenseState::Invalid
    }
}

/// Save activated license to disk
pub fn save_license(license_key: &str) -> Result<(), String> {
    let device_id = get_device_id();
    let data = LicenseData {
        license_key: license_key.to_string(),
        device_id,
        activated_at: chrono::Local::now().to_rfc3339(),
        version: LICENSE_VERSION,
    };
    
    let path = get_license_file_path();
    
    // Create directory if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {}", e))?;
    }
    
    // Encode data (simple base64 to prevent casual editing)
    let json = serde_json::to_string(&data).map_err(|e| format!("Failed to serialize: {}", e))?;
    let encoded = base64::encode(json.as_bytes());
    
    fs::write(&path, encoded).map_err(|e| format!("Failed to save license: {}", e))?;
    
    Ok(())
}

/// Load saved license from disk
fn load_saved_license() -> Option<LicenseData> {
    let path = get_license_file_path();
    let encoded = fs::read_to_string(&path).ok()?;
    let decoded = base64::decode(encoded.trim()).ok()?;
    let json = String::from_utf8(decoded).ok()?;
    serde_json::from_str(&json).ok()
}

/// Get license file path
fn get_license_file_path() -> PathBuf {
    let mut path = dirs_next::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."));
    path.push("InterviewHelper");
    path.push(".license");
    path
}

// =============================================================================
// 🔑 LICENSE GENERATION (Same algorithm used in generator tool)
// =============================================================================

/// Internal license generation - same as generator uses
fn generate_license_internal(device_id: &str) -> String {
    // Combine device ID with secret
    let combined = format!("{}:{}:{}", device_id, LICENSE_SECRET, LICENSE_VERSION);
    
    // Multi-round hashing for security
    let mut hash = hash_string(&combined);
    for _ in 0..1000 {
        hash = hash_string(&format!("{}:{}", hash, LICENSE_SECRET));
    }
    
    // Format as license key: LICENSE-XXXX-XXXX-XXXX-XXXX
    let hex = format!("{:016X}", hash);
    format!(
        "SM-{}-{}-{}-{}",
        &hex[0..4],
        &hex[4..8],
        &hex[8..12],
        &hex[12..16]
    )
}

/// Hash helper
fn hash_string(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// Get the expected license for current device (FOR DEBUGGING ONLY - remove in production!)
#[cfg(debug_assertions)]
pub fn debug_get_expected_license() -> String {
    let device_id = get_device_id();
    generate_license_internal(&device_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_license_generation_consistent() {
        let device_id = "TEST-1234-5678-ABCD";
        let license1 = generate_license_internal(device_id);
        let license2 = generate_license_internal(device_id);
        assert_eq!(license1, license2);
    }
    
    #[test]
    fn test_license_format() {
        let device_id = "TEST-1234-5678-ABCD";
        let license = generate_license_internal(device_id);
        assert!(license.starts_with("SM-"));
        assert_eq!(license.len(), 22); // SM-XXXX-XXXX-XXXX-XXXX
    }
}
