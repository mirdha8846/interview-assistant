use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    
    let api_key = std::env::var("GOOGLE_API_KEY")
        .expect("GOOGLE_API_KEY env var not set");

    let model = "models/gemini-2.5-flash-native-audio-preview-12-2025";
    let url = "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent";
    let ws_url = format!("{}?key={}", url, api_key);

    println!("🔗 Testing Complex Setup Message...");
    
    let (ws_stream, _) = connect_async(&ws_url).await?;
    let (mut write, mut read) = ws_stream.split();

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
            "inputAudioTranscription": {},
            "outputAudioTranscription": {},
            "systemInstruction": {
                "parts": [
                    {
                        "text": "You are a helpful assistant."
                    }
                ]
            }
        }
    });
    
    write.send(Message::Text(setup_msg.to_string())).await?;
    
    if let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => println!("📨 Received: {}", text),
            Ok(Message::Binary(bin)) => println!("📨 Received (bin): {}", String::from_utf8_lossy(&bin)),
            Ok(Message::Close(frame)) => println!("🔴 Closed: {:?}", frame),
            _ => println!("📨 Other message"),
        }
    }

    Ok(())
}
