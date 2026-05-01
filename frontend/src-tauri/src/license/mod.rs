//! License Management Module
//! 
//! Handles:
//! - Hardware fingerprinting (Device ID)
//! - License key verification
//! - Activation state management

pub mod fingerprint;
pub mod verify;

pub use fingerprint::get_device_id;
pub use verify::{verify_license, is_licensed, save_license, LicenseState};
