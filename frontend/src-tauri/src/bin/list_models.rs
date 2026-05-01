use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file
    dotenv::dotenv().ok();
    
    let api_key = env::var("GOOGLE_API_KEY")
        .expect("GOOGLE_API_KEY env var not set");

    println!("🔍 Fetching available models...\n");

    // List models endpoint
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
        api_key
    );

    let resp = reqwest::get(&url).await?;
    let status = resp.status();
    let body = resp.text().await?;

    if !status.is_success() {
        println!("❌ Error: {} - {}", status, body);
        return Ok(());
    }

    let json: serde_json::Value = serde_json::from_str(&body)?;
    
    println!("✅ Available Models:\n");
    println!("{:<50} | {:<15} | Live API", "Model Name", "Version");
    println!("{}", "-".repeat(80));

    if let Some(models) = json.get("models").and_then(|m| m.as_array()) {
        for model in models {
            let name = model.get("name").and_then(|n| n.as_str()).unwrap_or("?");
            let version = model.get("version").and_then(|v| v.as_str()).unwrap_or("-");
            
            // Check if model supports Live API (look for specific patterns)
            let supports_live = name.contains("native-audio") || 
                               name.contains("live") ||
                               model.get("supportedGenerationMethods")
                                   .and_then(|m| m.as_array())
                                   .map(|arr| arr.iter().any(|v| 
                                       v.as_str().map(|s| s.contains("bidi") || s.contains("stream")).unwrap_or(false)
                                   ))
                                   .unwrap_or(false);
            
            let live_marker = if supports_live { "✅ Yes" } else { "-" };
            
            // Highlight native-audio models
            if name.contains("native-audio") || name.contains("flash-exp") {
                println!("🔥 {:<47} | {:<15} | {}", name, version, live_marker);
            } else {
                println!("   {:<47} | {:<15} | {}", name, version, live_marker);
            }
        }
    }

    println!("\n📌 Use model names with 🔥 for Live API (BidiGenerateContent)");

    Ok(())
}
