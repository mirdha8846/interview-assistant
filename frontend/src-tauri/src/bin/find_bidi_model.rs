use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    
    let api_key = env::var("GOOGLE_API_KEY")
        .expect("GOOGLE_API_KEY env var not set");

    println!("🔍 Fetching models and supported methods...\n");

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
        api_key
    );

    let resp = reqwest::get(&url).await?;
    let body = resp.text().await?;
    let json: serde_json::Value = serde_json::from_str(&body)?;

    if let Some(models) = json.get("models").and_then(|m| m.as_array()) {
        for model in models {
            let name = model.get("name").and_then(|n| n.as_str()).unwrap_or("?");
            let methods = model.get("supportedGenerationMethods")
                .and_then(|m| m.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                .unwrap_or_default();
            
            if methods.iter().any(|m| m.contains("bidiGenerateContent")) {
                println!("✅ Supported: {}", name);
                println!("   Methods: {:?}", methods);
            } else if name.contains("flash") || name.contains("native-audio") {
                println!("❌ Not Supported: {}", name);
                println!("   Methods: {:?}", methods);
            }
        }
    }

    Ok(())
}
