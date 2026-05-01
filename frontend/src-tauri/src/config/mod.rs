//! Configuration Management
//! 
//! Handles:
//! - API keys storage (encrypted)
//! - User profile/resume
//! - App settings

pub mod api_keys;
pub mod user_profile;

pub use api_keys::{ApiKeys, load_api_keys, save_api_keys, are_api_keys_configured};
pub use user_profile::{UserProfile, load_user_profile, save_user_profile, get_ai_context};
