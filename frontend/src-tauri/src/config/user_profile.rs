//! User Profile Management
//! 
//! Stores user's resume/introduction for personalized AI responses

use std::path::PathBuf;
use std::fs;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct UserProfile {
    // Basic Info
    pub name: String,
    pub email: String,
    pub phone: String,
    
    // Professional Info
    pub current_role: String,
    pub experience_years: u32,
    pub skills: Vec<String>,
    pub technologies: Vec<String>,
    
    // Resume/Bio
    pub summary: String,       // Brief professional summary
    pub resume_text: String,   // Full resume content (paste from PDF)
    
    // Interview Prep
    pub target_role: String,          // Role they're interviewing for
    pub target_company: String,       // Company name (optional)
    pub interview_notes: String,      // Any specific notes for this interview
    
    // Preferences
    pub response_style: ResponseStyle,
    pub include_examples: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub enum ResponseStyle {
    #[default]
    Balanced,     // Mix of technical and conversational
    Technical,    // More code-focused, detailed
    Concise,      // Short, to-the-point answers
    Detailed,     // Comprehensive explanations
}

impl UserProfile {
    pub fn is_configured(&self) -> bool {
        // Return true if any meaningful info is present
        !self.name.is_empty() || !self.interview_notes.is_empty() || !self.summary.is_empty()
    }
    
    /// Generate skills string for AI context
    pub fn skills_string(&self) -> String {
        self.skills.join(", ")
    }
    
    /// Generate technologies string for AI context
    pub fn tech_string(&self) -> String {
        self.technologies.join(", ")
    }
}

/// Get AI context from user profile for personalized responses
pub fn get_ai_context() -> String {
    let profile = load_user_profile();
    if profile.is_configured() {
        let style_instruction = match profile.response_style {
                ResponseStyle::Balanced => "Give balanced answers mixing technical depth with clear explanations.",
                ResponseStyle::Technical => "Give highly technical answers with code examples and deep details.",
                ResponseStyle::Concise => "Give brief, direct answers. Be concise.",
                ResponseStyle::Detailed => "Give comprehensive, detailed explanations covering all aspects.",
            };
            
            let examples_note = if profile.include_examples {
                "Include relevant examples from your experience when appropriate."
            } else {
                ""
            };
            
            format!(
r#"
=== CANDIDATE PROFILE (You are answering AS this person) ===

Name: {}
Current Role: {}
Experience: {} years
Skills: {}
Technologies: {}

Professional Summary:
{}

{}

Target Role: {}
{}

=== CRITICAL INTERVIEW CONTEXT & CUSTOM RULES ===
{}

=== RESPONSE INSTRUCTIONS ===
- Answer questions AS this candidate, using first person ("I", "my experience")
- Reference their actual skills and experience when relevant
- {} 
{}
- Keep answers interview-appropriate (professional but personable)
- If asked about experience you don't have, be honest but pivot to related skills
"#,
                profile.name,
                profile.current_role,
                profile.experience_years,
                profile.skills_string(),
                profile.tech_string(),
                profile.summary,
                if !profile.resume_text.is_empty() {
                    format!("Resume Details:\n{}", profile.resume_text)
                } else {
                    String::new()
                },
                profile.target_role,
                if !profile.target_company.is_empty() {
                    format!("Company: {}", profile.target_company)
                } else {
                    String::new()
                },
                profile.interview_notes,
                style_instruction,
                examples_note
            )
    } else {
        String::new() // No profile configured
    }
}

/// Load user profile from storage
#[tauri::command]
pub fn load_user_profile() -> UserProfile {
    let path = get_profile_path();
    if let Ok(content) = fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        UserProfile::default()
    }
}

/// Save user profile to storage
#[tauri::command]
pub fn save_user_profile(profile: UserProfile) -> Result<(), String> {
    let path = get_profile_path();
    
    // Create directory if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {}", e))?;
    }
    
    let json = serde_json::to_string_pretty(&profile)
        .map_err(|e| format!("Failed to serialize: {}", e))?;
    
    fs::write(&path, json).map_err(|e| format!("Failed to save profile: {}", e))?;
    
    // Notify AI module to reload context
    crate::ai::live_client::reload_context();
    
    Ok(())
}

/// Get profile file path
fn get_profile_path() -> PathBuf {
    let mut path = dirs_next::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."));
    path.push("InterviewHelper");
    path.push("profile.json");
    path
}

/// Quick update for interview-specific notes (before each interview)
#[tauri::command]
pub fn update_interview_context(target_role: String, target_company: String, notes: String) -> Result<(), String> {
    let mut profile = load_user_profile();
    profile.target_role = target_role;
    profile.target_company = target_company;
    profile.interview_notes = notes;
    save_user_profile(profile)
}
