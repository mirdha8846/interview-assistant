//! AssemblyAI v3 (Universal Streaming) Optimized Service
//!
//! Features:
//! - Raw Binary Streaming (Message::Binary)
//! - Ultra-low latency voice agent settings
//! - Authorization header handshake

use std::sync::Arc;
use tokio::sync::mpsc;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use std::sync::atomic::{AtomicBool, Ordering};

pub static IS_STT_CONNECTED: AtomicBool = AtomicBool::new(false);

/// AssemblyAI v3 Response Message
#[derive(serde::Deserialize, Debug)]
pub struct AssemblyResponse {
    #[serde(rename = "type")]
    pub message_type: String,
    pub transcript: Option<String>,
    pub end_of_turn: Option<bool>,
    pub utterance: Option<String>,
    pub error: Option<String>,
    pub turn_order: Option<u32>,
    pub end_of_turn_confidence: Option<f64>,
}

/// Connect to AssemblyAI v3 Optimized and run the session
pub async fn connect_and_run(
    api_key: &str,
    on_transcript: impl Fn(String, bool, u32) + Send + Sync + Clone + 'static,
    mut audio_rx: mpsc::Receiver<Vec<u8>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    IS_STT_CONNECTED.store(false, Ordering::SeqCst);

    // 🎯 OPTIMIZED: max_turn_silence=200ms for fast response
    // Quick turn detection for interview speed
    let query = "sample_rate=16000&encoding=pcm_s16le&format_turns=false&end_of_turn_confidence_threshold=0.3&min_end_of_turn_silence_when_confident=50&max_turn_silence=200";
    let url = format!("wss://streaming.assemblyai.com/v3/ws?{}", query);
    
    crate::log_info!("🔗 Connecting to AssemblyAI v3 (Optimized for long interviews)...");

    // 🎯 FIX: Manually build the WebSocket handshake request
    let key: String = tokio_tungstenite::tungstenite::handshake::client::generate_key();
    
    let request = http::Request::builder()
        .method("GET")
        .uri(&url)
        .header("Host", "streaming.assemblyai.com")
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Key", key)
        .header("Sec-WebSocket-Version", "13")
        .header("Authorization", api_key)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .body(())?;

    let (ws_stream, _) = connect_async(request).await?;
    crate::log_info!("✅ AssemblyAI v3 connected!");

    let (write, mut read) = ws_stream.split();
    let on_transcript = Arc::new(on_transcript);
    
    // 🎯 Wrap writer in Arc<Mutex> so both tasks can use it
    let write = Arc::new(tokio::sync::Mutex::new(write));
    let write_audio = write.clone();
    let write_ping = write.clone();

    let session_result = tokio::select! {
        // == READER TASK ==
        read_res = async {
            let mut msgs_total = 0;
            while let Some(msg) = read.next().await {
                if !crate::audio::IS_LIVE_STREAMING.load(Ordering::SeqCst) {
                    break;
                }

                match msg {
                    Ok(Message::Text(text)) => {
                        msgs_total += 1;

                        match serde_json::from_str::<AssemblyResponse>(&text) {
                            Ok(resp) => {
                                match resp.message_type.as_str() {
                                    "Begin" => {
                                        crate::log_info!("🚀 Session v3 Started Successfully");
                                        IS_STT_CONNECTED.store(true, Ordering::SeqCst);
                                    }
                                    "Turn" => {
                                        if let Some(t) = resp.transcript {
                                            if !t.is_empty() {
                                                let turn_num = resp.turn_order.unwrap_or(0);
                                                on_transcript(t, resp.end_of_turn.unwrap_or(false), turn_num);
                                            }
                                        }
                                    }
                                    "Error" => {
                                        let err = resp.error.unwrap_or_else(|| "Unknown error".to_string());
                                        crate::log_error!("❌ AssemblyAI Error: {}", err);
                                        return Err(format!("v3 API Error: {}", err));
                                    }
                                    "Termination" => {
                                        crate::log_info!("🛑 Session Terminated");
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            Err(e) => {
                                crate::log_error!("❌ JSON Parse Fail Context: {}. Raw: {}", e, text);
                            }
                        }
                    }
                    Ok(Message::Pong(_)) => {
                        // Keep-alive pong received - connection is healthy
                    }
                    Ok(Message::Close(frame)) => {
                        crate::log_info!("🔌 WS Closed: {:?}", frame);
                        break;
                    }
                    Err(e) => return Err(format!("v3 Read Error: {:?}", e)),
                    _ => {}
                }
            }
            Ok::<_, String>(())
        } => read_res.map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() }),
        
        // == 🔄 KEEP-ALIVE PING TASK (Prevents 30+ minute silence disconnection) ==
        ping_res = async {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(15));
            loop {
                interval.tick().await;
                if !crate::audio::IS_LIVE_STREAMING.load(Ordering::SeqCst) {
                    break;
                }
                
                // Send WebSocket ping frame
                let mut writer = write_ping.lock().await;
                if let Err(e) = writer.send(Message::Ping(vec![0x01])).await {
                    crate::log_error!("❌ Ping failed: {:?}", e);
                    return Err(format!("Ping Error: {:?}", e));
                }
                
                // Also send 100ms of silence audio to keep AssemblyAI session alive
                // 16kHz * 0.1s * 2 bytes = 3200 bytes of silence
                let silence = vec![0u8; 3200];
                if let Err(e) = writer.send(Message::Binary(silence)).await {
                    crate::log_error!("❌ Silence keepalive failed: {:?}", e);
                    return Err(format!("Keepalive Error: {:?}", e));
                }
            }
            Ok::<_, String>(())
        } => ping_res.map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() }),

        // == WRITER TASK ==
        write_res = async {
            let mut bytes_sent = 0;
            let mut chunks_sent = 0;
            let mut audio_buffer = Vec::with_capacity(3200);
            let target_size = 1600; // 50ms @ 16kHz S16LE
            
            while crate::audio::IS_LIVE_STREAMING.load(Ordering::SeqCst) {
                match audio_rx.recv().await {
                    Some(pcm) => {
                        if pcm.is_empty() { continue; }
                        
                        audio_buffer.extend_from_slice(&pcm);
                        
                        while audio_buffer.len() >= target_size {
                            let chunk: Vec<u8> = audio_buffer.drain(0..target_size).collect();
                            bytes_sent += chunk.len();
                            chunks_sent += 1;

                            let mut writer = write_audio.lock().await;
                            if let Err(e) = writer.send(Message::Binary(chunk)).await {
                                return Err(format!("v3 Write Error: {:?}", e));
                            }
                            drop(writer); // Release lock quickly
                            
                            if chunks_sent % 100 == 0 {
                                crate::log_info!("📤 Sent {} chunks ({} KB)", chunks_sent, bytes_sent / 1024);
                            }
                        }
                    }
                    None => break,
                }
            }
            Ok::<_, String>(())
        } => write_res.map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() }),
    };

    IS_STT_CONNECTED.store(false, Ordering::SeqCst);
    
    // Optional: Send graceful termination message
    {
        let mut writer = write.lock().await;
        let _ = writer.send(Message::Text(json!({"type": "Terminate"}).to_string())).await;
        let _ = writer.close().await;
    }
    session_result
}
