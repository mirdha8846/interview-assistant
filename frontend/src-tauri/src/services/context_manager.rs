//! Context Manager Service
//! 
//! Manages interview session context, Q&A history, and candidate profile.
//! Rust equivalent of services/contextManager.js

use std::sync::Mutex;
use serde::{Deserialize, Serialize};

/// Single Q&A history entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QAEntry {
    pub question: String,
    pub answer: String,
    pub timestamp: u64,
}

/// Candidate profile for personalized responses
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CandidateProfile {
    pub resume: String,
    pub role: String,
    pub company: String,
    pub skills: Vec<String>,
    pub experience: String,
    pub notes: String,
}

/// Session settings
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionSettings {
    /// Pause duration in milliseconds to consider utterance complete
    pub pause_duration_ms: u64,
    /// Whether to auto-generate responses (false = manual via hotkey)
    pub auto_generate: bool,
    /// Maximum Q&A history items to keep
    pub max_history_items: usize,
}

impl Default for SessionSettings {
    fn default() -> Self {
        Self {
            pause_duration_ms: 450,
            auto_generate: false, // Manual-only by default
            max_history_items: 50,
        }
    }
}

/// Context Manager - owns session state
pub struct ContextManager {
    session_id: Option<String>,
    history: Vec<QAEntry>,
    profile: CandidateProfile,
    settings: SessionSettings,
}

impl ContextManager {
    pub fn new() -> Self {
        Self {
            session_id: None,
            history: Vec::new(),
            profile: CandidateProfile::default(),
            settings: SessionSettings::default(),
        }
    }

    // ========================
    // SESSION MANAGEMENT
    // ========================

    /// Start a new session, returns session ID
    pub fn start_session(&mut self) -> String {
        let id = format!("{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis());
        
        self.session_id = Some(id.clone());
        self.history.clear();
        self.load_profile_from_storage();
        
        crate::log_info!("📋 Session started: {}", id);
        id
    }

    /// End current session
    pub fn end_session(&mut self) {
        self.save_session_to_storage();
        if let Some(id) = self.session_id.take() {
            crate::log_info!("📋 Session ended: {}", id);
        }
    }

    /// Get current session ID
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Check if session is active
    pub fn is_active(&self) -> bool {
        self.session_id.is_some()
    }

    // ========================
    // HISTORY MANAGEMENT
    // ========================

    /// Add a Q&A pair to history
    pub fn add_to_history(&mut self, question: &str, answer: &str) -> QAEntry {
        let entry = QAEntry {
            question: question.to_string(),
            answer: answer.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        };

        self.history.push(entry.clone());

        // Trim if too long
        if self.history.len() > self.settings.max_history_items {
            let start = self.history.len() - self.settings.max_history_items;
            self.history = self.history[start..].to_vec();
        }

        crate::log_info!("📝 Added to history (total: {})", self.history.len());
        entry
    }

    /// Get recent history entries
    pub fn get_recent_history(&self, count: usize) -> Vec<&QAEntry> {
        let start = self.history.len().saturating_sub(count);
        self.history[start..].iter().collect()
    }

    /// Get all history
    pub fn get_all_history(&self) -> &[QAEntry] {
        &self.history
    }

    /// Clear history
    pub fn clear_history(&mut self) {
        self.history.clear();
        crate::log_info!("📝 History cleared");
    }

    /// Get history count
    pub fn history_count(&self) -> usize {
        self.history.len()
    }

    // ========================
    // PROFILE MANAGEMENT
    // ========================

    /// Update candidate profile
    pub fn update_profile(&mut self, profile: CandidateProfile) {
        self.profile = profile;
        self.save_profile_to_storage();
    }

    /// Get current profile reference
    pub fn profile(&self) -> &CandidateProfile {
        &self.profile
    }

    /// Get context for AI prompts
    pub fn get_ai_context(&self) -> String {
        let mut ctx = String::new();

        if !self.profile.role.is_empty() {
            ctx.push_str(&format!("Role: {}\n", self.profile.role));
        }
        if !self.profile.company.is_empty() {
            ctx.push_str(&format!("Company: {}\n", self.profile.company));
        }
        if !self.profile.skills.is_empty() {
            ctx.push_str(&format!("Skills: {}\n", self.profile.skills.join(", ")));
        }
        if !self.profile.experience.is_empty() {
            ctx.push_str(&format!("Experience: {}\n", self.profile.experience));
        }

        // Add recent Q&A context
        let recent = self.get_recent_history(3);
        if !recent.is_empty() {
            ctx.push_str("\nRecent Q&A:\n");
            for entry in recent {
                let q_short: String = entry.question.chars().take(80).collect();
                let a_short: String = entry.answer.chars().take(80).collect();
                ctx.push_str(&format!("Q: {}...\nA: {}...\n", q_short, a_short));
            }
        }

        ctx
    }

    // ========================
    // SETTINGS
    // ========================

    /// Update settings
    pub fn update_settings(&mut self, settings: SessionSettings) {
        self.settings = settings;
        self.save_settings_to_storage();
    }

    /// Get current settings
    pub fn settings(&self) -> &SessionSettings {
        &self.settings
    }

    // ========================
    // PERSISTENCE (Stub - uses log dir)
    // ========================

    fn get_storage_path() -> Option<std::path::PathBuf> {
        // Use a temp/log directory for persistence
        let mut path = std::env::temp_dir();
        path.push("interview-copilot");
        std::fs::create_dir_all(&path).ok()?;
        Some(path)
    }

    fn save_profile_to_storage(&self) {
        if let Some(mut path) = Self::get_storage_path() {
            path.push("profile.json");
            if let Ok(json) = serde_json::to_string_pretty(&self.profile) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    fn load_profile_from_storage(&mut self) {
        if let Some(mut path) = Self::get_storage_path() {
            path.push("profile.json");
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(profile) = serde_json::from_str(&data) {
                    self.profile = profile;
                    crate::log_info!("📋 Profile loaded from storage");
                }
            }
        }
    }

    fn save_settings_to_storage(&self) {
        if let Some(mut path) = Self::get_storage_path() {
            path.push("settings.json");
            if let Ok(json) = serde_json::to_string_pretty(&self.settings) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    #[allow(dead_code)]
    fn load_settings_from_storage(&mut self) {
        if let Some(mut path) = Self::get_storage_path() {
            path.push("settings.json");
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(mut settings) = serde_json::from_str::<SessionSettings>(&data) {
                    // Enforce manual-only mode
                    settings.auto_generate = false;
                    self.settings = settings;
                    crate::log_info!("📋 Settings loaded from storage");
                }
            }
        }
    }

    fn save_session_to_storage(&self) {
        if let (Some(mut path), Some(session_id)) = (Self::get_storage_path(), &self.session_id) {
            path.push(format!("session_{}.json", session_id));
            
            #[derive(Serialize)]
            struct SessionData<'a> {
                session_id: &'a str,
                history: &'a [QAEntry],
            }
            
            let data = SessionData {
                session_id,
                history: &self.history,
            };
            
            if let Ok(json) = serde_json::to_string_pretty(&data) {
                let _ = std::fs::write(path, json);
            }
        }
    }
}

// Global context manager instance
lazy_static::lazy_static! {
    pub static ref CONTEXT: Mutex<ContextManager> = Mutex::new(ContextManager::new());
}

// Public convenience functions
pub fn start_session() -> String {
    CONTEXT.lock().map(|mut c| c.start_session()).unwrap_or_default()
}

pub fn end_session() {
    if let Ok(mut c) = CONTEXT.lock() {
        c.end_session();
    }
}

pub fn add_qa(question: &str, answer: &str) {
    if let Ok(mut c) = CONTEXT.lock() {
        c.add_to_history(question, answer);
    }
}

pub fn get_ai_context() -> String {
    CONTEXT.lock().map(|c| c.get_ai_context()).unwrap_or_default()
}
