//! Setup Wizard Module
//! 
//! Handles first-time setup and configuration UI:
//! - License activation (one-time)
//! - Interview setup (every time before app start)
//! - API keys configuration
//! - User profile setup

pub mod wizard;
pub mod interview_setup;

pub use wizard::{run_setup_wizard, SetupResult};
pub use interview_setup::show_interview_setup;
