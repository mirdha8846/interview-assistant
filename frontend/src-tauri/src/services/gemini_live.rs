//! Gemini Live Service
//! 
//! Pure BidiGenerateContent WebSocket implementation.
//! Rust equivalent of services/geminiLive.js
//! 
//! Features:
//! - WebSocket connection management
//! - Audio queueing before setup complete
//! - Keepalive for long silences
//! - Input/output transcription callbacks
//! - Auto-reconnection with backoff

use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

/// Connection state enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ConnectionState {
    Disconnected = 0,
    Connecting = 1,
    Connected = 2,
    Reconnecting = 3,
    FatalError = 4,
}

impl From<u8> for ConnectionState {
    fn from(val: u8) -> Self {
        match val {
            0 => Self::Disconnected,
            1 => Self::Connecting,
            2 => Self::Connected,
            3 => Self::Reconnecting,
            4 => Self::FatalError,
            _ => Self::Disconnected,
        }
    }
}

impl ConnectionState {
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Disconnected => "🔴",
            Self::Connecting => "🟡",
            Self::Connected => "🟢",
            Self::Reconnecting => "🟠",
            Self::FatalError => "💀",
        }
    }
}

/// Callbacks for Gemini Live events
pub struct GeminiLiveCallbacks {
    /// Called with AI's answer text (streaming chunks)
    pub on_answer: Option<Box<dyn Fn(String) + Send + Sync>>,
    /// Called with input (interviewer) transcription
    pub on_input_transcription: Option<Box<dyn Fn(String) + Send + Sync>>,
    /// Called when model turn completes
    pub on_turn_complete: Option<Box<dyn Fn() + Send + Sync>>,
    /// Called on errors
    pub on_error: Option<Box<dyn Fn(String) + Send + Sync>>,
    /// Called when connection status changes
    pub on_status_change: Option<Box<dyn Fn(ConnectionState) + Send + Sync>>,
}

impl Default for GeminiLiveCallbacks {
    fn default() -> Self {
        Self {
            on_answer: None,
            on_input_transcription: None,
            on_turn_complete: None,
            on_error: None,
            on_status_change: None,
        }
    }
}

/// Audio level info for UI feedback
#[derive(Debug, Clone, Copy, Default)]
pub struct AudioLevel {
    pub rms: f32,
    pub peak: f32,
}

/// Configuration for the Gemini Live service
pub struct GeminiLiveConfig {
    pub api_key: String,
    pub model: String,
    pub system_instruction: String,
    pub keepalive_interval_ms: u64,
    pub keepalive_min_idle_ms: u64,
    pub max_queued_audio_chunks: usize,
}

impl Default for GeminiLiveConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "models/gemini-2.5-flash-native-audio-latest".to_string(),
            system_instruction: DEFAULT_SYSTEM_INSTRUCTION.to_string(),
            keepalive_interval_ms: 5000,
            keepalive_min_idle_ms: 3500,
            max_queued_audio_chunks: 60,
        }
    }
}

const DEFAULT_SYSTEM_INSTRUCTION: &str = r#"
You are an expert real-time interview assistant.

RULES:
- Respond immediately
- Speak naturally and confidently
- 4–6 sentences
- No meta language
- First word must be speakable
- Reference earlier context only if relevant
"#;

// Global state
static CONNECTION_STATE: AtomicU8 = AtomicU8::new(0);
static LAST_AUDIO_SENT_AT: AtomicU64 = AtomicU64::new(0);
static IS_SETUP_COMPLETE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Get current connection state
pub fn get_connection_state() -> ConnectionState {
    ConnectionState::from(CONNECTION_STATE.load(Ordering::SeqCst))
}

/// Set connection state
fn set_connection_state(state: ConnectionState) {
    CONNECTION_STATE.store(state as u8, Ordering::SeqCst);
}

/// Check if setup is complete
pub fn is_setup_complete() -> bool {
    IS_SETUP_COMPLETE.load(Ordering::SeqCst)
}

/// Gemini Live Service
pub struct GeminiLiveService {
    config: GeminiLiveConfig,
    callbacks: Arc<RwLock<GeminiLiveCallbacks>>,
    audio_queue: Arc<Mutex<Vec<Vec<u8>>>>,
    audio_tx: Option<mpsc::Sender<Vec<u8>>>,
}

impl GeminiLiveService {
    pub fn new(config: GeminiLiveConfig) -> Self {
        Self {
            config,
            callbacks: Arc::new(RwLock::new(GeminiLiveCallbacks::default())),
            audio_queue: Arc::new(Mutex::new(Vec::new())),
            audio_tx: None,
        }
    }

    /// Set callbacks
    pub async fn set_callbacks(&self, callbacks: GeminiLiveCallbacks) {
        *self.callbacks.write().await = callbacks;
    }

    /// Set API key
    pub fn set_api_key(&mut self, key: String) {
        self.config.api_key = key;
    }

    /// Set model
    pub fn set_model(&mut self, model: String) {
        self.config.model = model;
    }

    /// Send audio (queues if not ready)
    pub async fn send_audio(&self, pcm_buffer: Vec<u8>) {
        if pcm_buffer.is_empty() {
            return;
        }

        // If not setup complete, queue audio
        if !is_setup_complete() {
            let mut queue = self.audio_queue.lock().await;
            if queue.len() < self.config.max_queued_audio_chunks {
                queue.push(pcm_buffer);
            }
            return;
        }

        // Send via channel if available
        if let Some(ref tx) = self.audio_tx {
            let _ = tx.send(pcm_buffer).await;
        }
    }

    /// Build setup message
    fn build_setup_message(&self) -> serde_json::Value {
        json!({
            "setup": {
                "model": &self.config.model,
                "generationConfig": {
                    "responseModalities": ["AUDIO"],
                    "temperature": 0.2
                },
                "systemInstruction": {
                    "parts": [{ "text": &self.config.system_instruction }]
                },
                "outputAudioTranscription": {},
                "inputAudioTranscription": {}
            }
        })
    }

    /// Helper: encode PCM to base64
    fn encode_audio(pcm: &[u8]) -> String {
        use base64::{Engine as _, engine::general_purpose};
        general_purpose::STANDARD.encode(pcm)
    }

    /// Build audio message
    fn build_audio_message(pcm: &[u8]) -> serde_json::Value {
        json!({
            "realtimeInput": {
                "audio": {
                    "mimeType": "audio/pcm;rate=16000",
                    "data": Self::encode_audio(pcm)
                }
            }
        })
    }

    /// Build text message
    #[allow(dead_code)]
    fn build_text_message(text: &str) -> serde_json::Value {
        json!({
            "clientContent": {
                "turns": [{
                    "role": "user",
                    "parts": [{ "text": text }]
                }],
                "turnComplete": true
            }
        })
    }
}

// ============================================
// STANDALONE CONNECT FUNCTION
// (More flexible for the current architecture)
// ============================================

/// Connect to Gemini Live and run the session
/// Returns when session ends or errors out
pub async fn connect_and_run(
    api_key: &str,
    model: &str,
    system_instruction: &str,
    on_answer: impl Fn(String) + Send + Sync + Clone + 'static,
    audio_rx: mpsc::Receiver<Vec<u8>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    set_connection_state(ConnectionState::Connecting);
    IS_SETUP_COMPLETE.store(false, Ordering::SeqCst);

    let url = "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent";
    let ws_url = format!("{}?key={}", url, api_key);

    crate::log_info!("🔗 Connecting to Gemini Live (model: {})...", model);

    // Connect with timeout
    let connect_result = tokio::time::timeout(
        tokio::time::Duration::from_secs(30),
        connect_async(&ws_url),
    ).await;

    let (ws_stream, _) = match connect_result {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => {
            set_connection_state(ConnectionState::Disconnected);
            return Err(format!("WebSocket connect failed: {:?}", e).into());
        }
        Err(_) => {
            set_connection_state(ConnectionState::Disconnected);
            return Err("Connection timeout (30s)".into());
        }
    };

    crate::log_info!("✅ WebSocket connected!");

    let (mut write, mut read) = ws_stream.split();

    // Send setup message
    let setup_msg = json!({
        "setup": {
            "model": model,
            "generationConfig": {
                "responseModalities": ["AUDIO"],
                "speechConfig": {
                    "voiceConfig": {
                        "prebuiltVoiceConfig": {
                            "voiceName": "Puck"
                        }
                    }
                }
            },
            "systemInstruction": {
                "parts": [{ "text": system_instruction }]
            },
            "outputAudioTranscription": {},
            "inputAudioTranscription": {}
        }
    });

    write.send(Message::Text(setup_msg.to_string())).await?;
    crate::log_info!("📡 Setup message sent");

    // Wait for setup complete
    let setup_result = tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        read.next()
    ).await;

    match setup_result {
        Ok(Some(Ok(Message::Text(t)))) => {
            if t.contains("setupComplete") {
                IS_SETUP_COMPLETE.store(true, Ordering::SeqCst);
                set_connection_state(ConnectionState::Connected);
                crate::log_info!("✅ Setup complete!");
            }
        }
        Ok(Some(Ok(Message::Binary(b)))) => {
            if let Ok(t) = String::from_utf8(b) {
                if t.contains("setupComplete") {
                    IS_SETUP_COMPLETE.store(true, Ordering::SeqCst);
                    set_connection_state(ConnectionState::Connected);
                    crate::log_info!("✅ Setup complete!");
                }
            }
        }
        Ok(Some(Ok(Message::Close(f)))) => {
            return Err(format!("Server closed on setup: {:?}", f).into());
        }
        Ok(Some(Err(e))) => return Err(format!("Setup error: {:?}", e).into()),
        Ok(None) => return Err("Connection closed during setup".into()),
        Err(_) => return Err("Setup timeout".into()),
        // Ignore Ping, Pong, Frame messages during setup
        _ => {}
    }

    // Now run the main read/write loops
    let on_answer = Arc::new(on_answer);
    let on_answer_clone = on_answer.clone();
    let mut audio_rx = audio_rx;

    let session_result = tokio::select! {
        // Read loop
        read_res = async {
            while let Some(msg) = read.next().await {
                if !crate::audio::IS_LIVE_STREAMING.load(Ordering::SeqCst) {
                    break;
                }

                match msg {
                    Ok(Message::Text(text)) => {
                        handle_server_message(&text, &on_answer_clone);
                    }
                    Ok(Message::Binary(data)) => {
                        if let Ok(text) = String::from_utf8(data) {
                            handle_server_message(&text, &on_answer_clone);
                        }
                    }
                    Ok(Message::Close(frame)) => {
                        return Err(format!("Server closed: {:?}", frame));
                    }
                    Err(e) => {
                        return Err(format!("Read error: {:?}", e));
                    }
                    _ => {}
                }
            }
            Ok::<_, String>(())
        } => read_res.map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() }),

        // Write loop
        write_res = async {
            let mut audio_buffer: Vec<u8> = Vec::with_capacity(32000);
            let buffer_target = 6400; // ~200ms at 16kHz mono

            loop {
                if !crate::audio::IS_LIVE_STREAMING.load(Ordering::SeqCst) {
                    return Ok::<_, String>(());
                }

                tokio::select! {
                    chunk = audio_rx.recv() => {
                        match chunk {
                            Some(pcm) => {
                                audio_buffer.extend_from_slice(&pcm);

                                if audio_buffer.len() >= buffer_target {
                                    let msg = GeminiLiveService::build_audio_message(&audio_buffer);
                                    if let Err(e) = write.send(Message::Text(msg.to_string())).await {
                                        return Err(format!("Send error: {:?}", e));
                                    }
                                    LAST_AUDIO_SENT_AT.store(
                                        std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_millis() as u64,
                                        Ordering::SeqCst
                                    );
                                    audio_buffer.clear();
                                }
                            }
                            None => {
                                return Err("Audio channel closed".to_string());
                            }
                        }
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {}
                }
            }
        } => write_res.map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() }),
    };

    // Cleanup
    set_connection_state(ConnectionState::Disconnected);
    IS_SETUP_COMPLETE.store(false, Ordering::SeqCst);
    let _ = write.close().await;

    session_result
}

/// Handle incoming server message
fn handle_server_message(text: &str, on_answer: &Arc<impl Fn(String) + Send + Sync + 'static>) {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(content) = v.get("serverContent") {
            // Output transcription (model's spoken words as text)
            if let Some(output_trans) = content.get("outputTranscription") {
                if let Some(text_val) = output_trans.get("text").and_then(|t| t.as_str()) {
                    if !text_val.trim().is_empty() {
                        on_answer(text_val.to_string());
                    }
                }
            }

            // Input transcription (interviewer's speech)
            if let Some(input_trans) = content.get("inputTranscription") {
                if let Some(text_val) = input_trans.get("text").and_then(|t| t.as_str()) {
                    if !text_val.trim().is_empty() {
                        crate::log_info!("🎤 Input: {}", text_val);
                    }
                }
            }

            // Model turn parts (fallback text)
            if let Some(model_turn) = content.get("modelTurn") {
                if let Some(parts) = model_turn.get("parts").and_then(|p| p.as_array()) {
                    for part in parts {
                        if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                            if !t.trim().is_empty() {
                                on_answer(t.to_string());
                            }
                        }
                    }
                }
            }

            // Turn complete
            if let Some(tc) = content.get("turnComplete") {
                if tc.as_bool().unwrap_or(false) {
                    crate::log_info!("🔄 Turn complete");
                }
            }
        }

        // Check for errors
        if let Some(err) = v.get("error") {
            let msg = err.get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");
            crate::log_error!("❌ Server error: {}", msg);
        }
    }
}
