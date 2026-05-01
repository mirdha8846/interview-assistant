use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Configuration
    let api_key = env::var("GOOGLE_API_KEY").unwrap_or_else(|_| "your_api".to_string());
    
    // START with the user's suggested URL (v1alpha)
    let url = "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateContent";
    let ws_url = format!("{}?key={}", url, api_key);

    println!("🔗 Connecting to: {}", url);
    
    // 2. Connect
    let (ws_stream, response) = connect_async(&ws_url).await.expect("Failed to connect");
    println!("✅ WebSocket Handshake HTTP Response: {:?}", response.status());
    
    let (mut write, mut read) = ws_stream.split();

    // 3. Setup Message
    let setup_msg = json!({
        "setup": {
            "model": "models/gemini-2.0-flash-exp", 
            "generationConfig": {
                "responseModalities": ["AUDIO", "TEXT"],
            }
        }
    });
    
    println!("outbox -> {}", setup_msg);
    write.send(Message::Text(setup_msg.to_string())).await?;

    // 4. Read Response
    println!("Waiting for response...");
    if let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => println!("📨 Received: {}", text),
            Ok(Message::Binary(bin)) => println!("📨 Received (bin): {:?}", String::from_utf8_lossy(&bin)),
            Ok(Message::Close(data)) => println!("🔴 Closed: {:?}", data),
            Err(e) => println!("❌ Error: {:?}", e),
            _ => println!("📨 Other message received"),
        }
    } else {
        println!("🔴 Connection closed without response");
    }

    Ok(())
}
