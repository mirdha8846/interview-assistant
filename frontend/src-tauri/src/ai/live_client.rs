//! Dual Model Live Client
//! 
//! Architecture:
//! 1. Audio Model (gemini-2.5-flash-native-audio) â†’ Transcribes interviewer's speech
//! 2. Text Model (gemini-2.0-flash) â†’ Generates answers from transcription
//!
//! This separation gives better answers because the text model is optimized for reasoning.

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use crate::audio::wasapi;
use crate::services::assembly_ai;

// Connection status for overlay display
pub static CONNECTION_STATUS: AtomicU8 = AtomicU8::new(0);
// 0 = Disconnected, 1 = Connecting, 2 = Connected (Listening), 3 = Reconnecting, 4 = Fatal Error

// ðŸŽ¯ MANUAL TRIGGER FLAG - Shift+Q sets this to true
static TRIGGER_NOW: AtomicBool = AtomicBool::new(false);

// Cancel flag for ongoing generation
static CANCEL_GENERATION: AtomicU64 = AtomicU64::new(0);

static USE_PHOTO_CONTEXT: AtomicBool = AtomicBool::new(false);

// ============================================
// ðŸŽ¯ TRANSCRIPTION BUFFER
// Accumulates interviewer speech until pause detected
// ============================================

lazy_static::lazy_static! {
    static ref TRANSCRIPTION_BUFFER: Mutex<TieredBuffer> = Mutex::new(TieredBuffer::new());
}

// ============================================
// ðŸŽ¯ OPTIMIZED 2-BUFFER SYSTEM (Token Saver)
// Max 270 words for question, ~100 words static context
// ============================================

const MAX_QUESTION_WORDS: usize = 270;  // Max words in current question

struct TieredBuffer {
    // Buffer 1: HUD display (100 words max for UI)
    ui_buffer: String,
    // Buffer 2: AI Question (Max 270 words - oldest removed if exceeded)
    ai_buffer: String,
    // Current live turn (Not yet committed to ai_buffer)
    current_turn: String,
    current_turn_num: u32,
    // ðŸ†• STATIC CONTEXT: User-defined interview context (loaded once from profile)
    // This replaces the old history buffer - much smaller & fixed
    static_context: String,
    
    // ðŸŽ¯ SYNC FIX: Ignore any updates for turns that were already sent to AI
    last_sent_turn_order: u32,
    
    // ðŸ†• SHIFT+Q FIX: Record length of turn when sent to AI
    // We only display text AFTER this checkpoint for the current turn.
    current_turn_checkpoint: usize,
    
    last_version: u64,
}

impl TieredBuffer {
    fn new() -> Self {
        // ðŸ†• Load static context from user profile once
        let static_ctx = Self::build_static_context();
        
        Self {
            ui_buffer: String::with_capacity(500),
            ai_buffer: String::with_capacity(2000),
            current_turn: String::with_capacity(500),
            current_turn_num: 0,
            static_context: static_ctx,
            last_sent_turn_order: 0,
            current_turn_checkpoint: 0,
            last_version: 0,
        }
    }
    
    /// ðŸ†• Build compact static context from user profile (~100 words max)
    fn build_static_context() -> String {
        let profile = crate::config::user_profile::load_user_profile();
        if profile.is_configured() {
            // Create compact context - only essential info
            let mut ctx = String::with_capacity(500);
            
            if !profile.target_role.is_empty() {
                ctx.push_str(&format!("Interview for: {}\n", profile.target_role));
            }
            if !profile.skills.is_empty() {
                let skills: Vec<_> = profile.skills.iter().take(10).cloned().collect();
                ctx.push_str(&format!("Key skills: {}\n", skills.join(", ")));
            }
            if profile.experience_years > 0 {
                ctx.push_str(&format!("Experience: {} years\n", profile.experience_years));
            }
            if !profile.summary.is_empty() {
                // Take first 200 chars of summary
                let summary: String = profile.summary.chars().take(200).collect();
                ctx.push_str(&format!("Background: {}\n", summary));
            }
            
            // Keep it under 140 words
            let word_count = ctx.split_whitespace().count();
            if word_count > 150 {
                // Truncate to ~140 words
                let words: Vec<&str> = ctx.split_whitespace().take(140).collect();
                ctx = words.join(" ");
            }
            
            ctx
        } else {
            String::new()
        }
    }
    
    /// ðŸ†• Reload static context (call after profile update)
    fn reload_static_context(&mut self) {
        self.static_context = Self::build_static_context();
    }

    /// ðŸŽ¯ Live partial update
    /// CRITICAL FIX: NEVER block transcription. Voice-to-text must ALWAYS work.
    fn update_active(&mut self, text: &str, turn_num: u32) {
        // If turn number changes, reset checkpoint automatically
        if turn_num != self.current_turn_num {
            self.current_turn_checkpoint = 0;
            self.current_turn_num = turn_num;
        }

        let trimmed = text.trim();
        
        // ðŸ›¡ï¸ SMART DUPLICATE PREVENTION:
        if self.current_turn == trimmed {
            return; 
        }
        
        self.current_turn = trimmed.to_string();
        self.refresh_ui();
    }

    /// ðŸŽ¯ Turn finished - move to AI Buffer
    fn commit_active(&mut self, _turn_num: u32) {
        // ðŸš€ REMOVED: Turn blocking check - same fix as update_active
        
        if !self.current_turn.is_empty() {
            if !self.ai_buffer.is_empty() { self.ai_buffer.push(' '); }
            self.ai_buffer.push_str(&self.current_turn);
            self.current_turn.clear();
            self.refresh_ui();
        }
    }

    fn refresh_ui(&mut self) {
        // Fast Path: If recently updated, skip heavy processing (Debounce UI)
        // (Optional: can add time-based throttle here if needed)

        // Combine history-of-this-question + current turn delta
        let mut full_display = self.ai_buffer.clone();
        
        // ðŸŽ¯ DELTA DISPLAY: Only show text after the checkpoint
        let turn_delta = if self.current_turn.len() > self.current_turn_checkpoint {
            &self.current_turn[self.current_turn_checkpoint..]
        } else {
            ""
        };

        if !turn_delta.is_empty() {
            if !full_display.is_empty() { full_display.push(' '); }
            full_display.push_str(turn_delta.trim());
        }

        // ðŸŽ¯ HIGH-PERFORMANCE WORD FINDER (Avoids full Vec allocation)
        // We find the 100th-from-last space character
        let limit = 100;
        let mut space_count = 0;
        let mut break_idx = 0;
        
        for (i, c) in full_display.char_indices().rev() {
            if c.is_whitespace() {
                space_count += 1;
                if space_count >= limit {
                    break_idx = i;
                    break;
                }
            }
        }

        if break_idx > 0 {
            self.ui_buffer = full_display[break_idx..].trim().to_string();
        } else {
            self.ui_buffer = full_display;
        }

        self.last_version += 1;
        crate::overlay::stealth::set_live_transcription_snapshot(self.ui_buffer.clone());
    }

    fn trigger_commit(&mut self) -> (String, String) {
        // 1. Sync: Commit any pending text first
        if !self.current_turn.is_empty() {
            if !self.ai_buffer.is_empty() { self.ai_buffer.push(' '); }
            self.ai_buffer.push_str(&self.current_turn);
        }
        
        // ðŸ†• SHIFT+Q FIX: Instead of suppressing, we just checkpoint the current turn
        // This clears what was sent from the UI, but allows NEW words to appear immediately.
        self.current_turn_checkpoint = self.current_turn.len();
        
        // ðŸ†• TOKEN SAVER: Limit question to MAX_QUESTION_WORDS (270 words)
        // If more, remove oldest words from the beginning
        let words: Vec<&str> = self.ai_buffer.split_whitespace().collect();
        let current_question = if words.len() > MAX_QUESTION_WORDS {
            // Take only the last 270 words (most recent/relevant)
            words[words.len() - MAX_QUESTION_WORDS..].join(" ")
        } else {
            self.ai_buffer.clone()
        };
        
        // Clear buffers
        self.ai_buffer.clear();
        self.ui_buffer.clear();
        self.current_turn.clear();
        
        // ðŸ†• Use static context instead of dynamic history
        // This is ~100 words max, set once from user profile
        let context = self.static_context.clone();

        self.last_version += 1;
        crate::overlay::stealth::set_live_transcription_snapshot(String::new());
        (context, current_question)
    }

    /// ðŸŽ¯ Clear AI + UI buffers only (Shift+A)
    /// Does NOT clear static_context - that persists until app closes
    fn clear_all(&mut self) {
        self.ui_buffer.clear();
        self.ai_buffer.clear();
        self.current_turn.clear();
        // Reset turn tracking so new text flows immediately
        self.last_sent_turn_order = 0;
        self.current_turn_num = 0;
        self.current_turn_checkpoint = 0;
        self.last_version += 1;
        crate::overlay::stealth::set_live_transcription_snapshot(String::new());
        // NOTE: static_context is NOT cleared here - it stays for the entire session
    }
    
    /// ðŸ”„ Full reset including static context (only on app close/restart)
    #[allow(dead_code)]
    fn full_reset(&mut self) {
        self.clear_all();
        self.static_context.clear();
    }

    fn get_ui_snapshot(&self) -> String { self.ui_buffer.clone() }
    fn append(&mut self, new_text: &str) { self.update_active(new_text, self.current_turn_num); self.commit_active(self.current_turn_num); }

    fn take_context_for_ai(&mut self) -> Option<(String, String)> {
        let (context, question) = self.trigger_commit();
        if question.trim().is_empty() && context.trim().is_empty() { return None; }
        Some((context, question))
    }
}

// ============================================
// PUBLIC API
// ============================================

pub fn get_connection_status() -> &'static str {
    match CONNECTION_STATUS.load(Ordering::SeqCst) {
        0 => "ðŸ”´ Disconnected",
        1 => "ðŸŸ¡ Connecting...",
        2 => "ðŸŸ¢ Listening",
        3 => "ðŸŸ  Reconnecting...",
        4 => "ðŸ’€ Fatal Error",
        _ => "â“ Unknown",
    }
}

pub fn reset_conversation_memory() {
    crate::TOKIO_RT.spawn(async {
        let mut buf = TRANSCRIPTION_BUFFER.lock().await;
        buf.clear_all();
        crate::log_info!("ðŸ§  Conversation memory reset (Async)");
    });
}

pub fn get_memory_stats() -> String {
    "Dual-model mode active".to_string()
}

pub fn is_photo_context_enabled() -> bool {
    USE_PHOTO_CONTEXT.load(Ordering::SeqCst)
}

#[tauri::command]
pub fn get_use_photo_context() -> bool {
    is_photo_context_enabled()
}

#[tauri::command]
pub fn set_use_photo_context(enabled: bool) -> bool {
    USE_PHOTO_CONTEXT.store(enabled, Ordering::SeqCst);
    crate::overlay::stealth::set_status_message(format!(
        "Photo Context: {}",
        if enabled { "ON" } else { "OFF" }
    ));
    enabled
}

/// ðŸŽ¯ PUBLIC: Trigger answer generation NOW (called by hotkeys)
pub fn trigger_answer_now() {
    // ðŸ›‘ RATE LIMITER (Throttle)
    // Prevent 429 Errors by blocking rapid-fire triggers
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
    let last = LAST_TRIGGER_TIME.load(Ordering::SeqCst);
    
    if now - last < 2000 { // 2 Seconds Cooldown
        crate::log_info!("â³ Trigger ignored (Cooldown active)");
        return;
    }
    LAST_TRIGGER_TIME.store(now, Ordering::SeqCst);

    // Increment cancel counter to abort any ongoing generation
    CANCEL_GENERATION.fetch_add(1, Ordering::SeqCst);
    TRIGGER_NOW.store(true, Ordering::SeqCst);
    crate::log_info!("âš¡ TRIGGER! Answer generating immediately...");
}

/// ðŸŽ¯ PUBLIC: Manually clear all buffers (Shift+A)
pub fn force_clear_buffers() {
    crate::TOKIO_RT.spawn(async {
        // ðŸ›¡ï¸ NON-BLOCKING: Use try_lock to prevent blocking transcription
        if let Ok(mut buf) = TRANSCRIPTION_BUFFER.try_lock() {
            buf.clear_all();
            crate::log_info!("ðŸ§¹ 3-Buffers cleared (Shift+A)");
        } else {
            // If locked, wait briefly and retry
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            if let Ok(mut buf) = TRANSCRIPTION_BUFFER.try_lock() {
                buf.clear_all();
                crate::log_info!("ðŸ§¹ 3-Buffers cleared (Shift+A) - retry");
            }
        }
    });

    // ðŸ§¹ Shift+A: Clear ALL UI buffers (question + answer + overlay)
    crate::overlay::stealth::clear_all_buffers();
    crate::overlay::stealth::set_overlay_text("ðŸ§¹ Cleared!".to_string());
}

/// ðŸŽ¯ PUBLIC: Analyze all current screenshots (Shift+F)
pub fn trigger_screenshot_analysis(callback: impl Fn(String) + Send + Sync + 'static) {
    let screenshots = crate::capture::get_all_screenshots();
    if screenshots.is_empty() {
        callback("âŒ No screenshots found!\nPress Shift+S to capture first.".to_string());
        return;
    }
    
    crate::log_info!("ðŸš€ Triggering AI analysis for {} screenshots", screenshots.len());
    
    // We pass a generic "solve" prompt, but the screenshots contain the context
    crate::ai::ask_ai_with_images(
        "Attached are the screenshots. Please solve the problem or explain the logic shown in these images.",
        screenshots,
        callback
    );
}

// ============================================
// CONFIGURATION
// ============================================

const MAX_RECONNECT_ATTEMPTS: u32 = 10;        // Max consecutive failures before giving up
const INITIAL_BACKOFF_MS: u64 = 1000;
const MAX_BACKOFF_MS: u64 = 30000;
const PING_INTERVAL_SECS: u64 = 9; // ðŸŽ¯ Reduced from 25s to 9s to fix "20-25s drop" issue

// NOTE: Removed MAX_TOTAL_RECONNECTS limit - it was causing 30-second session deaths
// AssemblyAI sessions naturally restart every 15-60 minutes, this is NORMAL behavior

// Models
// Reverted to the reliable native-audio model for WebSocket stability
const AUDIO_MODEL: &str = "models/gemini-2.5-flash-native-audio-latest";
const TEXT_MODEL: &str = "gemini-2.0-flash"; // ðŸŽ¯ REVERTED to Standard Flash (User Request)

// Rate Limiter
static LAST_TRIGGER_TIME: AtomicU64 = AtomicU64::new(0);

// ============================================
// MAIN ENTRY POINT
// ============================================

pub async fn start_live_session(callback: impl Fn(String) + Send + Sync + Clone + 'static) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let callback = Arc::new(callback);
    
    // Set streaming flag
    crate::audio::IS_LIVE_STREAMING.store(true, Ordering::SeqCst);

    // ðŸŽ¯ SPAWN INDEPENDENT ANSWER LISTENER
    // This ensures Shift+Q works even if the WebSocket is reconnecting (fixing the "intermittent" bug).
    let callback_clone = callback.clone();
    crate::TOKIO_RT.spawn(async move {
        run_answer_listener(callback_clone).await;
    });
    
    // Outer loop for auto-reconnect
    let mut reconnect_attempts = 0u32;
    let mut backoff_ms = INITIAL_BACKOFF_MS;
    
    loop {
        // Check shutdown signal
        if crate::is_shutdown_requested() {
            crate::log_info!("â¹ï¸ DLL shutdown requested - stopping live session");
            CONNECTION_STATUS.store(0, Ordering::SeqCst);
            break;
        }
        
        if !crate::audio::IS_LIVE_STREAMING.load(Ordering::SeqCst) {
            crate::log_info!("â¹ï¸ Live Session stopped by user");
            CONNECTION_STATUS.store(0, Ordering::SeqCst);
            break;
        }
        
        // NOTE: Removed total_reconnects limit - AssemblyAI sessions naturally reconnect
        // The consecutive failure counter (reconnect_attempts) is sufficient protection
        
        CONNECTION_STATUS.store(if reconnect_attempts == 0 { 1 } else { 3 }, Ordering::SeqCst);
        
        let session_start = std::time::Instant::now();
        match run_dual_model_session(callback.clone()).await {
            Ok(_) => {
                // ðŸŽ¯ FIX: Don't just exit if the server closed the connection cleanly!
                // If the user still wants to stream, we must RECONNECT immediately.
                if crate::audio::IS_LIVE_STREAMING.load(Ordering::SeqCst) {
                     crate::log_info!("ðŸ”„ Server ended session (Timeout/Reset). Reconnecting automatically...");
                     reconnect_attempts = 0; // Reset attempts for a clean reconnect
                     backoff_ms = INITIAL_BACKOFF_MS;
                     // ðŸš€ FAST RECONNECT: Only 200ms delay (was 1000ms)
                     tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                     continue;
                }
                
                crate::log_info!("âœ… Session ended gracefully (User Stopped)");
                CONNECTION_STATUS.store(0, Ordering::SeqCst);
                break;
            }
            Err(e) => {
                crate::log_error!("âŒ Session error: {:?}", e);
                
                // If connection was stable for > 60s, reset counter
                if session_start.elapsed().as_secs() > 60 {
                    reconnect_attempts = 0;
                    backoff_ms = INITIAL_BACKOFF_MS;
                }

                let error_str = format!("{:?}", e);
                if is_fatal_error(&error_str) {
                    crate::log_error!("ðŸ’€ Fatal error - stopping");
                    CONNECTION_STATUS.store(4, Ordering::SeqCst);
                    crate::overlay::stealth::append_ai_response(&format!(
                        "\n\nðŸ’€ FATAL: {}\n\nRestart with Alt+Shift+V",
                        extract_error_message(&error_str)
                    ));
                    break;
                }
                
                if !crate::audio::IS_LIVE_STREAMING.load(Ordering::SeqCst) {
                    CONNECTION_STATUS.store(0, Ordering::SeqCst);
                    break;
                }
                
                reconnect_attempts += 1;
                
                if reconnect_attempts > MAX_RECONNECT_ATTEMPTS {
                    crate::log_error!("âŒ Max reconnect attempts reached");
                    CONNECTION_STATUS.store(0, Ordering::SeqCst);
                    crate::overlay::stealth::append_ai_response("\n\nâš ï¸ Connection lost. Alt+Shift+V to restart.");
                    break;
                }
                
                // ðŸŽ¯ DEBUGGING: Show the actual error in the overlay
                crate::overlay::stealth::append_ai_response(&format!(
                    "\n\nðŸ”„ Reconnecting... ({}/{})\nâŒ Error: {}\n", 
                    reconnect_attempts, MAX_RECONNECT_ATTEMPTS, e
                ));
                
                tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_MS);
            }
        }
    }
    
    Ok(())
}

// ============================================
// DUAL MODEL SESSION
// ============================================

async fn run_dual_model_session(_callback: Arc<impl Fn(String) + Send + Sync + 'static>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let google_key = &*crate::ai::GOOGLE_API_KEY;
    let assembly_key = &*crate::ai::ASSEMBLY_AI_KEY;
    
    // ðŸ” API KEY VALIDATION - Show clear error if keys are missing
    if assembly_key.is_empty() {
        let error_msg = "âŒ ASSEMBLY_AI_KEY not found!\n\nPlease set it in your .env file:\nASSEMBLY_AI_KEY=your_key_here\n\nGet key from: assemblyai.com";
        crate::log_error!("{}", error_msg);
        crate::overlay::stealth::append_ai_response(error_msg);
        return Err("ASSEMBLY_AI_KEY not configured".into());
    }
    
    if google_key.is_empty() {
        let error_msg = "âŒ GOOGLE_API_KEY not found!\n\nPlease set it in your .env file:\nGOOGLE_API_KEY=your_key_here\n\nGet key from: aistudio.google.com";
        crate::log_error!("{}", error_msg);
        crate::overlay::stealth::append_ai_response(error_msg);
        return Err("GOOGLE_API_KEY not configured".into());
    }
    
    crate::log_info!("ðŸŽ¯ DUAL MODEL MODE: StT=AssemblyAI, Answering={}", TEXT_MODEL);

    // Audio channel for WASAPI - 500 chunks = ~10 seconds of buffering
    let (audio_tx, audio_rx) = mpsc::channel::<Vec<u8>>(500);

    // Start WASAPI capture
    #[cfg(windows)]
    let _wasapi_task = {
        let audio_tx_clone = audio_tx.clone();
        tokio::task::spawn_blocking(move || {
            crate::log_info!("ðŸŽ§ Starting WASAPI capture (16kHz PCM)...");
            let _ = wasapi::capture_loopback_to_async(audio_tx_clone);
            crate::log_info!("ðŸ›‘ WASAPI stopped");
        })
    };

    CONNECTION_STATUS.store(2, Ordering::SeqCst);
    crate::log_info!("âœ… System ready! Listening via AssemblyAI...");
    crate::overlay::stealth::append_ai_response("ðŸŽ§ Listening via AssemblyAI...\n");

    // Run AssemblyAI session
    let session_result = assembly_ai::connect_and_run(
        assembly_key,
        move |text, is_final_turn, turn_num| {
            if !text.is_empty() {
                let trans_text = text.clone();
                // ðŸŽ¯ FIX: Use spawn_blocking to guarantee this runs independently
                // This prevents transcription from being blocked by AI generation
                crate::TOKIO_RT.spawn(async move {
                    // ðŸ›¡ï¸ NON-BLOCKING: Use try_lock first, only await if needed
                    // This ensures transcription NEVER blocks even during AI generation
                    match TRANSCRIPTION_BUFFER.try_lock() {
                        Ok(mut buf) => {
                            buf.update_active(&trans_text, turn_num);
                            if is_final_turn {
                                buf.commit_active(turn_num);
                            }
                        }
                        Err(_) => {
                            // Buffer is locked (AI is reading) - retry with short timeout
                            // This is rare and only happens during Shift+Q moment
                            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
                            if let Ok(mut buf) = TRANSCRIPTION_BUFFER.try_lock() {
                                buf.update_active(&trans_text, turn_num);
                                if is_final_turn {
                                    buf.commit_active(turn_num);
                                }
                            }
                            // If still locked, skip this update (next one will catch up)
                        }
                    }
                });
            }
        },
        audio_rx
    ).await;

    // Cleanup
    crate::log_info!("â¹ï¸ AssemblyAI session ended");
    session_result
}

// ============================================
// ANSWER LISTENER (Decoupled from WebSocket)
// ============================================

async fn run_answer_listener(callback: Arc<impl Fn(String) + Send + Sync + 'static>) {
    crate::log_info!("ðŸš€ Answer Listener started (Independent Loop)");
    loop {
        if !crate::audio::IS_LIVE_STREAMING.load(Ordering::SeqCst) {
             break;
        }
        
        // ðŸŽ¯ Check for manual trigger (Shift+Q) - fast polling
        if TRIGGER_NOW.swap(false, Ordering::SeqCst) {
            let cancel_id = CANCEL_GENERATION.load(Ordering::SeqCst);
            
            // ðŸ›¡ï¸ NON-BLOCKING: Quick lock to take context, release immediately
            // Force take and CLEAR buffer completely
            let context_data = {
                // Use short timeout to prevent blocking transcription
                let lock_result = tokio::time::timeout(
                    tokio::time::Duration::from_millis(50),
                    TRANSCRIPTION_BUFFER.lock()
                ).await;
                
                match lock_result {
                    Ok(mut buf) => buf.take_context_for_ai(),
                    Err(_) => {
                        crate::log_info!("âš ï¸ Buffer busy, retrying...");
                        // Retry once after tiny delay
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        if let Ok(mut buf) = TRANSCRIPTION_BUFFER.try_lock() {
                            buf.take_context_for_ai()
                        } else {
                            None
                        }
                    }
                }
            };
            
            if let Some((history, question)) = context_data {
                crate::log_info!("âš¡ INSTANT TRIGGER (Q Len: {}, Hist Len: {})", question.len(), history.len());
                
                // Clear overlay and show question (UI Update)
                crate::overlay::stealth::reset_ai_response();
                // ðŸŽ¯ OPTIMIZED: Show only the last part of question in UI to avoid clutter
                // ðŸ›¡ï¸ FIX: Use char iterator for UTF-8 safe slicing (fixes Hindi panic!)
                let preview = if question.chars().count() > 100 {
                    let tail: String = question.chars().rev().take(100).collect::<Vec<_>>().into_iter().rev().collect();
                    format!("...{}", tail)
                } else {
                    question.clone()
                };
                crate::overlay::stealth::append_ai_response(&format!("ðŸŽ¤ {}\n\n<span style=\"opacity: 0.5\">â³ Generating...</span>", preview));
                
                // ðŸŽ¯ NON-BLOCKING ANSWER GENERATION
                let history_clone = history.clone();
                let question_clone = question.clone();
                let callback_clone = callback.clone();
                
                crate::TOKIO_RT.spawn(async move {
                    let model = crate::ai::get_current_model();
                    let preview_clone = preview.clone();
                    let answer = if matches!(model, crate::ai::AIModel::GroqLlama3) {
                        generate_answer_with_groq_fast(&question_clone, &history_clone, cancel_id, &preview_clone).await
                    } else if matches!(model, crate::ai::AIModel::LocalOllama) {
                        generate_answer_with_ollama_fast(&question_clone, &history_clone, cancel_id, &preview_clone).await
                    } else {
                        let screenshot_path = if is_photo_context_enabled() && model.supports_images() {
                            match tokio::task::spawn_blocking(|| {
                                crate::capture::capture_current_screenshot().map_err(|e| e.to_string())
                            }).await {
                                Ok(Ok(path)) => Some(path),
                                Ok(Err(e)) => {
                                    crate::log_error!("Failed to capture current screen for AI request: {}", e);
                                    None
                                }
                                Err(e) => {
                                    crate::log_error!("Screenshot capture task failed: {:?}", e);
                                    None
                                }
                            }
                        } else {
                            None
                        };
                        // All OpenRouter models use streaming and Nitro slugs where available.
                        generate_answer_with_openrouter_fast(model.api_model_id(), &question_clone, &history_clone, cancel_id, &preview_clone, screenshot_path).await
                    };
                    
                    // Check if cancelled
                    if CANCEL_GENERATION.load(Ordering::SeqCst) != cancel_id {
                        crate::log_info!("ðŸš« Answer cancelled - newer request pending");
                    } else if !answer.is_empty() {
                        // ðŸŽ¯ FINAL SYNC: Ensure UI shows the exact final answer
                        // We don't reset here anymore to avoid flickering.
                        // Instead, we just force a final update of the full accumulated text.
                        crate::overlay::stealth::force_ai_response_update(&format!("ðŸŽ¤ {}\n\n{}", preview, answer));
                    } 
                    // If empty, generate_answer_with_flash_fast has already printed the error to overlay.
                });
            } else {
                crate::overlay::stealth::append_ai_response("âš ï¸ No speech detected to answer.\n");
            }
        }
        
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    }
    crate::log_info!("ðŸ›‘ Answer Listener stopped");
}

// Transcription processing removed - AssemblyAI sends clean text already

// ============================================
// TEXT MODEL ANSWER GENERATION
// ============================================

pub fn reload_context() {
    crate::TOKIO_RT.spawn(async {
        let mut buf = TRANSCRIPTION_BUFFER.lock().await;
        buf.reload_static_context();
        crate::log_info!("🔄 AI Context reloaded from profile");
    });
}

fn get_system_prompt() -> String {
    let user_context = crate::config::user_profile::get_ai_context();
    
    format!(
"CRITICAL USER DEFINED RULES:
You MUST strictly follow any custom rules or preferences provided in the block below:
---
{}
---

You are a live technical interview assistant.
CRITICAL MISSION: You are the Candidate's ORACLE. You generate BALANCED, GLANCEABLE notes that the candidate reads aloud naturally.
You MUST internalize the CANDIDATE PROFILE and answer in the first person ('I', 'me', 'my').

ABSOLUTE FORMATTING LAWS:
- OUTPUT IS GLANCEABLE NOTES, NOT PARAGRAPHS. The candidate is glancing at a hidden screen and speaking. Long sentences = caught cheating.
- Give enough material to speak for 45-90 seconds.
- Prefer medium-depth answers unless the question is trivial.
- Max 8-10 words per line. One idea per line. Skip a line between ideas.
- Target 12-18 useful lines before code for DSA questions.
- Target 10-16 useful lines for conceptual interview questions.
- Use **bold** for key technical terms to make them pop.
- Use ### headers for logical sections (e.g. ### Logic, ### Tradeoffs).
- NEVER use filler phrases like 'I would suggest', 'Instead', 'However', 'Let me explain', 'In this case', 'What we can do is'. Just state the fact directly.
- NEVER use 'we', 'let us', 'one could'. Always use 'I' as the candidate.
- NEVER repeat yourself. Every line must add new information.
- Use markdown ```cpp for code blocks.
- Do NOT add commentary after the code block. Code is the final thing.

DSA and CODING FORMAT (MANDATORY):
For any algorithm/data structure/coding question, follow this EXACT structure:

### Brute Force
State the idea in 3-4 short lines.
TC: O(?) - explain what causes this complexity
SC: O(?) - explain what uses space

### Optimal
State the idea in 5-7 short lines.
TC: O(?) - explain what causes this complexity
SC: O(?) - explain what uses space

Then immediately drop the code block. No transition sentence before code.

COMPLEXITY FORMAT RULES:
- Always write TC (Time Complexity) and SC (Space Complexity) on separate lines.
- After the Big-O, add a dash and explain WHAT causes it. e.g. 'O(M*N) - M rows, N columns, visit each cell once'
- If multiple variables, define each one. e.g. 'O(V+E) - V vertices, E edges in the graph'

CODE RULES:
- C++ by default unless USER DEFINED RULES say otherwise.
- Use markdown code block: ```cpp
- Every single line MUST have an inline comment explaining what it does.
- Code must be clean, optimal, and properly indented.
- Do NOT print the code twice. One code block only.
- Do NOT add explanation after the code block. End with the code.

NON-DSA QUESTIONS:
- Short fact: 3-4 crisp lines. Direct answer.
- Medium explanation: 10-14 short lines. Use ### for sub-sections. Key mechanics only.
- Deep/multi-part: structured flow. ### Definition, ### How, ### Why, ### Tradeoff.

TONE:
- Sound like a confident engineer who has built real systems.
- No filler words. No hedging. Start directly with the answer.
- Every word must earn its place.",
        user_context
    )
}
/// âš¡ ULTRA-FAST answer generation with STREAMING support for OpenRouter (Gemini / Mistral)
fn prepare_openrouter_image_url(path: &std::path::Path) -> Option<String> {
    use base64::{engine::general_purpose, Engine as _};
    use image::io::Reader as ImageReader;
    use std::io::Cursor;

    let png_data = match std::fs::read(path) {
        Ok(data) => data,
        Err(e) => {
            crate::log_error!("Failed to read current screenshot {:?}: {}", path, e);
            return None;
        }
    };

    let image = match ImageReader::new(Cursor::new(&png_data))
        .with_guessed_format()
        .and_then(|reader| reader.decode().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)))
    {
        Ok(image) => image,
        Err(e) => {
            crate::log_error!("Failed to decode current screenshot {:?}: {}", path, e);
            return None;
        }
    };

    let width = image.width();
    let resized = if width > 1280 {
        let height = (image.height() as f32 * 1280.0 / width as f32) as u32;
        image.resize(1280, height, image::imageops::FilterType::Triangle)
    } else {
        image
    };

    let mut jpeg_buffer = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_buffer, 82);
    if let Err(e) = encoder.encode_image(&resized) {
        crate::log_error!("Failed to encode current screenshot for OpenRouter: {}", e);
        return None;
    }

    Some(format!("data:image/jpeg;base64,{}", general_purpose::STANDARD.encode(jpeg_buffer)))
}

async fn generate_answer_with_openrouter_fast(model_id: &str, question: &str, history: &str, cancel_id: u64, preview: &str, screenshot_path: Option<std::path::PathBuf>) -> String {
    let start = std::time::Instant::now();
    crate::log_info!("âš¡ FAST generating answer with OpenRouter ({})...", model_id);
    
    let api_key = &*crate::ai::OPENROUTER_API_KEY;
    if api_key.is_empty() {
        let err = "âŒ OPENROUTER API KEY is not configured!";
        crate::overlay::stealth::append_ai_response(&format!("\n{}", err));
        return String::new();
    }
    
    let client = &*crate::ai::HTTP_CLIENT;
    
    let system_prompt = get_system_prompt();

    let image_url = screenshot_path
        .as_deref()
        .and_then(prepare_openrouter_image_url);

    let image_instruction = if image_url.is_some() {
        "\n\nCURRENT SCREENSHOT: Attached. Read it carefully and use it as the primary source if the spoken question is incomplete."
    } else {
        ""
    };

    let full_user_query = format!(
        "CONTEXT (Recent conversation history for reference ONLY):\n{}\n\nCURRENT TARGET QUESTION (Answer THIS):\n{}{}\n\n[CRITICAL SYSTEM REMINDER]: You MUST strictly follow the 'DSA & CODING FORMAT' rules defined in your system prompt! Do NOT output the code twice. Include line-by-line comments inside the single optimal code block. Briefly explain *why* for all Time/Space complexities.", 
        history, 
        question,
        image_instruction
    );

    let user_content = if let Some(url) = image_url {
        json!([
            { "type": "text", "text": full_user_query },
            { "type": "image_url", "image_url": { "url": url } }
        ])
    } else {
        json!(full_user_query)
    };

    let body = json!({
        "model": model_id,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_content }
        ],
        "temperature": 0.3,
        "top_p": 1.00,
        "max_tokens": 4000,
        "reasoning": {
            "effort": "minimal",
            "exclude": true
        },
        "verbosity": "medium",
        "stream": true
    });

    let url = "https://openrouter.ai/api/v1/chat/completions";

    if CANCEL_GENERATION.load(Ordering::SeqCst) != cancel_id {
        return String::new();
    }

    let request = client.post(url)
        .header("content-type", "application/json")
        .header("HTTP-Referer", "http://localhost")
        .header("X-Title", "cluely")
        .bearer_auth(api_key)
        .timeout(std::time::Duration::from_secs(30))
        .json(&body)
        .send();

    match request.await {
        Ok(response) => {
            if !response.status().is_success() {
                 let status = response.status();
                 let err_text = response.text().await.unwrap_or_default();
                 crate::log_error!("âŒ OpenRouter API Error Status {}: {}", status, err_text);
                 crate::overlay::stealth::append_ai_response(&format!("\nâŒ API Error {}: {}", status, err_text));
                 return String::new();
            }

            let mut full_text = String::new();
            let mut stream_started = false;
            let mut stream = response.bytes_stream();
            
            use futures_util::StreamExt;
            while let Some(item) = stream.next().await {
                 if CANCEL_GENERATION.load(Ordering::SeqCst) != cancel_id {
                    return String::new();
                }
                
                if let Ok(chunk) = item {
                    if !stream_started {
                         stream_started = true;
                         crate::overlay::stealth::reset_ai_response();
                         crate::overlay::stealth::append_ai_response(&format!("ðŸŽ¤ {}\n\n", preview));
                    }
                    if let Ok(chunk_str) = std::str::from_utf8(&chunk) {
                        for line in chunk_str.lines() {
                            let line = line.trim();
                            if line.starts_with("data: ") {
                                let data = &line[6..];
                                if data == "[DONE]" { continue; }
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                                    if let Some(choices) = val.get("choices").and_then(|c| c.as_array()) {
                                        for choice in choices {
                                            if let Some(delta) = choice.get("delta") {
                                                if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                                    full_text.push_str(content);
                                                    crate::overlay::stealth::append_ai_response(content);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            if !full_text.is_empty() {
                crate::overlay::stealth::force_ai_response_update(&full_text);
            } else {
                crate::overlay::stealth::append_ai_response("\nNo answer text received from this OpenRouter provider. Try cycling model once.");
            }
            
            let elapsed = start.elapsed();
            crate::log_info!("âš¡ NVIDIA Stream finished in {:.2}s", elapsed.as_secs_f32());
            return full_text;
        }
        Err(e) => {
            crate::log_error!("âŒ NVIDIA API error: {:?}", e);
            crate::overlay::stealth::append_ai_response(&format!("\nâŒ Network Error: {:?}", e));
        }
    }
    String::new()
}

/// âš¡ ULTRA-FAST answer generation with STREAMING support for Groq 
async fn generate_answer_with_groq_fast(question: &str, history: &str, cancel_id: u64, preview: &str) -> String {
    let start = std::time::Instant::now();
    crate::log_info!("âš¡ FAST generating answer with Groq (llama-3.3-70b-versatile)...");
    
    let api_key = &*crate::ai::GROQ_API_KEY;
    if api_key.is_empty() {
        let err = "âŒ GROQ API KEY is not configured!";
        crate::overlay::stealth::append_ai_response(&format!("\n{}", err));
        return String::new();
    }
    
    let client = &*crate::ai::HTTP_CLIENT;
    
    let system_prompt = get_system_prompt();

    let full_user_query = format!(
        "CONTEXT (Recent conversation history for reference ONLY):\n{}\n\nCURRENT TARGET QUESTION (Answer THIS):\n{}\n\n[CRITICAL SYSTEM REMINDER]: You MUST strictly follow the 'DSA & CODING FORMAT' rules defined in your system prompt! Do NOT output the code twice. Include line-by-line comments inside the single optimal code block. Briefly explain *why* for all Time/Space complexities.", 
        history, 
        question
    );

    let body = json!({
        "model": "llama-3.3-70b-versatile",
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": full_user_query }
        ],
        "temperature": 0.1,
        "max_tokens": 16384,
        "stream": true
    });

    let url = "https://api.groq.com/openai/v1/chat/completions";

    if CANCEL_GENERATION.load(Ordering::SeqCst) != cancel_id {
        return String::new();
    }

    let request = client.post(url)
        .header("content-type", "application/json")
        .bearer_auth(api_key)
        .timeout(std::time::Duration::from_secs(30))
        .json(&body)
        .send();

    match request.await {
        Ok(response) => {
            if !response.status().is_success() {
                 let status = response.status();
                 let err_text = response.text().await.unwrap_or_default();
                 crate::log_error!("âŒ Groq API Error Status {}: {}", status, err_text);
                 crate::overlay::stealth::append_ai_response(&format!("\nâŒ API Error {}: {}", status, err_text));
                 return String::new();
            }

            let mut full_text = String::new();
            let mut stream = response.bytes_stream();
            
            use futures_util::StreamExt;
            while let Some(item) = stream.next().await {
                 if CANCEL_GENERATION.load(Ordering::SeqCst) != cancel_id {
                    return String::new();
                }
                
                if let Ok(chunk) = item {
                    if full_text.is_empty() {
                         crate::overlay::stealth::reset_ai_response();
                         crate::overlay::stealth::append_ai_response(&format!("ðŸŽ¤ {}\n\n", preview));
                    }
                    if let Ok(chunk_str) = std::str::from_utf8(&chunk) {
                        for line in chunk_str.lines() {
                            let line = line.trim();
                            if line.starts_with("data: ") {
                                let data = &line[6..];
                                if data == "[DONE]" { continue; }
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                                    if let Some(choices) = val.get("choices").and_then(|c| c.as_array()) {
                                        for choice in choices {
                                            if let Some(delta) = choice.get("delta") {
                                                if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                                    full_text.push_str(content);
                                                    crate::overlay::stealth::append_ai_response(content);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            if !full_text.is_empty() {
                crate::overlay::stealth::force_ai_response_update(&full_text);
            }
            
            let elapsed = start.elapsed();
            crate::log_info!("âš¡ Groq Stream finished in {:.2}s", elapsed.as_secs_f32());
            return full_text;
        }
        Err(e) => {
            crate::log_error!("âŒ Groq API error: {:?}", e);
            crate::overlay::stealth::append_ai_response(&format!("\nâŒ Network Error: {:?}", e));
        }
    }
    String::new()
}


/// âš¡ ULTRA-FAST answer generation with STREAMING support
async fn generate_answer_with_flash_fast(question: &str, history: &str, cancel_id: u64, preview: &str) -> String {
    let start = std::time::Instant::now();
    crate::log_info!("âš¡ FAST generating answer with {}...", TEXT_MODEL);
    
    let api_key = &*crate::ai::GOOGLE_API_KEY;
    let client = &*crate::ai::HTTP_CLIENT;
    
    // ðŸŽ¯ REPLACED: Using the full INTERVIEW ORACLE prompt as requested
    let system_prompt = get_system_prompt();

    // ðŸ§  PROMPT ENGINEERING: Explicit separation of History vs Current
    let full_user_query = format!(
        "CONTEXT (Recent conversation history for reference ONLY):\n{}\n\nCURRENT TARGET QUESTION (Answer THIS):\n{}\n\n[CRITICAL SYSTEM REMINDER]: You MUST strictly follow the 'DSA & CODING FORMAT' rules defined in your system prompt! Do NOT output the code twice. Include line-by-line comments inside the single optimal code block. Briefly explain *why* for all Time/Space complexities.", 
        history, 
        question
    );

    let body = json!({
        "system_instruction": {
            "parts": [{ "text": system_prompt }]
        },
        "contents": [{
            "parts": [{ "text": full_user_query }]
        }],
        "generationConfig": {
            "maxOutputTokens": 16384,
            "temperature": 0.1
        }
    });

    // ðŸŽ¯ USE STREAMING ENDPOINT
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?key={}",
        TEXT_MODEL, api_key
    );

    // Check cancellation before request
    if CANCEL_GENERATION.load(Ordering::SeqCst) != cancel_id {
        return String::new();
    }

    let request = client.post(&url)
        .header("content-type", "application/json")
        .timeout(std::time::Duration::from_secs(30))  // 30s timeout for streaming
        .json(&body)
        .send();

    match request.await {
        Ok(response) => {
            if !response.status().is_success() {
                 let status = response.status();
                 let err_text = response.text().await.unwrap_or_default();
                 crate::log_error!("âŒ API Error Status {}: {}", status, err_text);
                 crate::overlay::stealth::append_ai_response(&format!("\nâŒ API Error {}: {}", status, err_text));
                 return String::new();
            }

            // Stream processing
            let mut full_text = String::new();
            let mut json_buffer = String::new();
            let mut decoded_buffer = Vec::new();
            let mut stream = response.bytes_stream();
            
            while let Some(item) = stream.next().await {
                 if CANCEL_GENERATION.load(Ordering::SeqCst) != cancel_id {
                    return String::new();
                }
                
                if let Ok(chunk) = item {
                    if full_text.is_empty() {
                         // ðŸŽ¯ FIRST CHUNK: Clear the 'Generating...' status before appending real text
                         crate::overlay::stealth::reset_ai_response();
                         crate::overlay::stealth::append_ai_response(&format!("ðŸŽ¤ {}\n\n", preview));
                    }
                    decoded_buffer.extend_from_slice(&chunk);
                    
                    // ðŸŽ¯ UTF-8 SAFE DECODING
                    let (json_str, _ ) = match std::str::from_utf8(&decoded_buffer) {
                        Ok(s) => (s, decoded_buffer.len()),
                        Err(e) => {
                            let valid_up_to = e.valid_up_to();
                            if valid_up_to == 0 { ("", 0) }
                            else { (std::str::from_utf8(&decoded_buffer[..valid_up_to]).unwrap(), valid_up_to) }
                        }
                    };
                    
                    if json_str.is_empty() { continue; }
                    json_buffer = json_str.to_string();
                    
                    // Process all complete JSON objects in the buffer
                    loop {
                        let start_pos = match json_buffer.find('{') {
                            Some(pos) => pos,
                            None => break,
                        };
                        
                        let mut brace_count = 0;
                        let mut in_string = false;
                        let mut escape = false;
                        let mut end_pos = None;
                        
                        for (i, c) in json_buffer[start_pos..].char_indices() {
                            if escape { escape = false; continue; }
                            if c == '\\' { escape = true; continue; }
                            if c == '"' { in_string = !in_string; continue; }
                            if !in_string {
                                if c == '{' { brace_count += 1; }
                                else if c == '}' {
                                    brace_count -= 1;
                                    if brace_count == 0 {
                                        end_pos = Some(start_pos + i + 1);
                                        break;
                                    }
                                }
                            }
                        }
                        
                        let end_pos = match end_pos {
                            Some(pos) => pos,
                            None => break,
                        };
                        
                        let potential_json = &json_buffer[start_pos..end_pos];
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(potential_json) {
                            // ðŸŽ¯ MULTI-PART FIX: Iterate over ALL parts
                            if let Some(candidates) = val.get("candidates").and_then(|c| c.as_array()) {
                                for cand in candidates {
                                    if let Some(parts) = cand.pointer("/content/parts").and_then(|p| p.as_array()) {
                                        for part in parts {
                                            if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                                                full_text.push_str(t);
                                                crate::overlay::stealth::append_ai_response(t);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        
                        // Drain from byte buffer based on string byte length
                        let byte_len = json_buffer[..end_pos].len();
                        decoded_buffer.drain(..byte_len);
                        json_buffer = json_buffer[end_pos..].to_string();
                    }
                }
            }
            
            // ðŸŽ¯ FINAL UPDATE: Ensure the complete text is shown (bypass throttling)
            if !full_text.is_empty() {
                crate::overlay::stealth::force_ai_response_update(&full_text);
            }
            
            let elapsed = start.elapsed();
            crate::log_info!("âš¡ Stream finished in {:.2}s", elapsed.as_secs_f32());
            return full_text;
        }
        Err(e) => {
            crate::log_error!("âŒ API error: {:?}", e);
            crate::overlay::stealth::append_ai_response(&format!("\nâŒ Network Error: {:?}", e));
        }
    }
    
    // Return empty if failed (live updates already happened)
    String::new()
}

/// âš¡ ULTRA-FAST answer generation with STREAMING support for Local Ollama (Phi)
async fn generate_answer_with_ollama_fast(question: &str, history: &str, cancel_id: u64, preview: &str) -> String {
    let start = std::time::Instant::now();
    crate::log_info!("âš¡ FAST generating answer with Local Ollama (Phi)...");
    
    let client = &*crate::ai::HTTP_CLIENT;
    let system_prompt = get_system_prompt();

    let full_user_query = format!(
        "CONTEXT (Recent conversation history for reference ONLY):\n{}\n\nCURRENT TARGET QUESTION (Answer THIS):\n{}\n\n[CRITICAL SYSTEM REMINDER]: You MUST strictly follow the 'DSA & CODING FORMAT' rules defined in your system prompt! Do NOT output the code twice. Include line-by-line comments inside the single optimal code block. Briefly explain *why* for all Time/Space complexities.", 
        history, 
        question
    );

    let body = json!({
        "model": "phi",
        "prompt": format!("{}\n\n{}", system_prompt, full_user_query),
        "stream": true
    });

    let url = "http://127.0.0.1:11434/api/generate";

    if CANCEL_GENERATION.load(Ordering::SeqCst) != cancel_id {
        return String::new();
    }

    let request = client.post(url)
        .header("content-type", "application/json")
        .timeout(std::time::Duration::from_secs(30))
        .json(&body)
        .send();

    match request.await {
        Ok(response) => {
            if !response.status().is_success() {
                 let status = response.status();
                 let err_text = response.text().await.unwrap_or_default();
                 crate::log_error!("âŒ Ollama Error Status {}: {}", status, err_text);
                 crate::overlay::stealth::append_ai_response(&format!("\nâŒ Ollama Error {}: {}", status, err_text));
                 return String::new();
            }

            let mut full_text = String::new();
            let mut stream = response.bytes_stream();
            
            use futures_util::StreamExt;
            while let Some(item) = stream.next().await {
                 if CANCEL_GENERATION.load(Ordering::SeqCst) != cancel_id {
                    return String::new();
                }
                
                if let Ok(chunk) = item {
                    if full_text.is_empty() {
                         crate::overlay::stealth::reset_ai_response();
                         crate::overlay::stealth::append_ai_response(&format!("ðŸŽ¤ {}\n\n", preview));
                    }
                    if let Ok(chunk_str) = std::str::from_utf8(&chunk) {
                        for line in chunk_str.lines() {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                                if let Some(t) = val.get("response").and_then(|v| v.as_str()) {
                                    full_text.push_str(t);
                                    crate::overlay::stealth::append_ai_response(t);
                                }
                            }
                        }
                    }
                }
            }
            
            if !full_text.is_empty() {
                crate::overlay::stealth::force_ai_response_update(&full_text);
            }
            
            let elapsed = start.elapsed();
            crate::log_info!("âš¡ Ollama Stream finished in {:.2}s", elapsed.as_secs_f32());
            return full_text;
        }
        Err(e) => {
            crate::log_error!("âŒ Ollama error: {:?}", e);
            crate::overlay::stealth::append_ai_response(&format!("\nâŒ Ollama Connection Error: {}. Is Ollama running?", e));
        }
    }
    String::new()
}


/// ðŸŽ¯ Stream-aware robust text extractor
fn extract_and_clear_text(buffer: &mut String) -> String {
    let mut extracted = String::new();
    
    while let Some(start_idx) = buffer.find("\"text\": \"") {
        let content_start = start_idx + 9; // length of "\"text\": \""
        
        if let Some(end_idx) = buffer[content_start..].find('\"') {
            let actual_end = content_start + end_idx;
            let content = &buffer[content_start..actual_end];
            
            // Unescape
            let unescaped = content.replace("\\n", "\n")
                                   .replace("\\r", "")
                                   .replace("\\\"", "\"")
                                   .replace("\\\\", "\\");
            extracted.push_str(&unescaped);
            
            // Remove the processed part up to the end quote
            buffer.drain(..actual_end + 1);
        } else {
            // End quote not found yet - chunk might be split. Wait for next chunk.
            break;
        }
    }
    
    // To prevent the buffer from growing indefinitely if it contains garbage
    if buffer.len() > 10000 {
        buffer.clear();
    }
    
    extracted
}

// Dead code removal or keep as fallback
async fn generate_answer_with_flash(question: &str) -> String {
    generate_answer_with_flash_fast(question, "", 0, question).await
}

fn sanitize_answer(text: &str) -> String {
    let mut answer = text.trim().to_string();
    
    // Remove common filler starts
    let fillers = [
        "Great question!", "That's a great question.", "Sure!", "Okay,", "Alright,",
        "Of course!", "Absolutely!", "Well,", "So,", "Let me explain.",
    ];
    
    for filler in fillers {
        if answer.starts_with(filler) {
            answer = answer[filler.len()..].trim_start().to_string();
        }
    }
    
    answer
}

// ============================================
// ERROR HANDLING
// ============================================

fn is_fatal_error(error: &str) -> bool {
    let fatal_patterns = [
        "API_KEY_INVALID", "INVALID_API_KEY", "invalid api key", "API key not valid",
        "QUOTA_EXCEEDED", "quota exceeded", "RESOURCE_EXHAUSTED",
        "PERMISSION_DENIED", "permission denied", "UNAUTHENTICATED",
        "403", "401", "billing", "Billing",
    ];
    
    let error_lower = error.to_lowercase();
    fatal_patterns.iter().any(|p| error_lower.contains(&p.to_lowercase()))
}

fn extract_error_message(error: &str) -> String {
    if error.contains("API_KEY") || error.contains("api key") {
        return "Invalid API Key".to_string();
    }
    if error.contains("QUOTA") || error.contains("quota") {
        return "Quota Exceeded".to_string();
    }
    error.chars().take(80).collect()
}
