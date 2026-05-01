//! Services Module
//! 
//! This module provides a clean, modular architecture matching the JS services layer:
//! - context_manager: Session, profile, history, settings
//! - gemini_live: WebSocket BidiGenerateContent connection
//! - audio_capture: System audio capture abstraction
//! - transcription: Speech-to-text (via Gemini inputTranscription)

pub mod context_manager;
pub mod gemini_live;
pub mod audio_capture;
pub mod assembly_ai;

// Re-export main types for convenience
pub use context_manager::{ContextManager, CandidateProfile, SessionSettings, QAEntry};
pub use gemini_live::{GeminiLiveService, GeminiLiveCallbacks, ConnectionState};
pub use audio_capture::{AudioCaptureService, AudioLevel};
