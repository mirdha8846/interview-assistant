use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use std::borrow::Cow;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    
    let api_key = std::env::var("GOOGLE_API_KEY")
        .expect("GOOGLE_API_KEY env var not set");

    let model = "models/gemini-2.5-flash-native-audio-preview-12-2025";
    let url = "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent";
    let ws_url = format!("{}?key={}", url, api_key);

    println!("🔗 Testing HYBRID PROMPT Injection...");
    
    let (ws_stream, _) = connect_async(&ws_url).await?;
    let (mut write, mut read) = ws_stream.split();

    // 1. Setup
    let setup_msg = json!({
        "setup": {
            "model": model,
            "systemInstruction": { "parts": [ { "text": "You are a Ghostwriter. No headers. No meta-talk." } ] }
        }
    });
    write.send(Message::Text(setup_msg.to_string())).await?;
    
    if let Some(msg) = read.next().await { println!("📨 Setup: {:?}", msg?); }

    // 2. Initial Turn Instruction (The Hammer)
    let hybrid_instr = json!({
        "clientContent": {
            "turns": [{
                "role": "user",
                "parts": [{ "text": "INSTRUCTIONS: START YOUR RESPONSE WITH 'GHOSTWRITER-ACTIVE'. Then explain the event loop in 2 sentences. No headers." }]
            }],
            "turnComplete": true
        }
    });
    write.send(Message::Text(hybrid_instr.to_string())).await?;
    println!("📡 Hybrid turn sent.");

    while let Some(msg) = read.next().await {
        let msg = msg?;
        if let Message::Text(text) = msg {
            let val: serde_json::Value = serde_json::from_str(&text)?;
            if let Some(t) = val.pointer("/serverContent/modelTurn/parts/0/text") {
                let text_part: Cow<'_, str> = match t.as_str() {
                    Some(s) => Cow::Borrowed(s),
                    None => Cow::Owned(t.to_string()),
                };

                println!("🤖 AI: {}", text_part);
                if text_part.contains("GHOSTWRITER-ACTIVE") {
                    println!("✅ PROMPT ENFORCED!");
                    break;
                }
            }
            if let Some(_) = val.pointer("/serverContent/turnComplete") { break; }
        }
    }

    Ok(())
}
