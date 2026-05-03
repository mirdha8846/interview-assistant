use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use reqwest::Client; // Async client
use futures_util::StreamExt;
use serde_json::json;
use std::sync::{Arc, Mutex};

pub mod live_client;

// Import user profile for personalized responses
use crate::config::user_profile::get_ai_context;

// =============================================================================
// 🔐 API KEY MANAGEMENT - SECURITY CRITICAL
// =============================================================================
// API keys are loaded ONLY from environment variables or .env file.
// NEVER hardcode API keys in source code - they will be extracted from binary.
// 
// Required environment variables:
//   - CLAUDE_API_KEY: Anthropic Claude API key
//   - GOOGLE_API_KEY: Google Gemini API key  
//   - GROQ_API_KEY: Groq Fast API key
//   - NVIDIA_API_KEY: Nvidia API key
//   - ASSEMBLY_AI_KEY: AssemblyAI transcription key
//   - SCREENSHOT_API_KEY: Google API key for screenshot analysis
//
// Setup: Create a .env file in the project root with these keys.
// =============================================================================

// Guard to prevent multiple simultaneous AI queries
static AI_QUERY_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

// Helper function to load API key with proper error logging
fn load_api_key(key_name: &str) -> String {
    dotenv::dotenv().ok();
    match std::env::var(key_name) {
        Ok(key) if !key.is_empty() => key,
        Ok(_) => {
            eprintln!("⚠️  WARNING: {} is set but empty!", key_name);
            String::new()
        }
        Err(_) => {
            eprintln!("❌ CRITICAL: {} not found in environment!", key_name);
            eprintln!("   Please set {} in your .env file or environment variables.", key_name);
            String::new()
        }
    }
}

// Load API keys from environment only - NO HARDCODED FALLBACKS
lazy_static::lazy_static! {
    static ref CLAUDE_API_KEY: String = load_api_key("CLAUDE_API_KEY");
    pub static ref GOOGLE_API_KEY: String = load_api_key("GOOGLE_API_KEY");
    pub static ref GROQ_API_KEY: String = load_api_key("GROQ_API_KEY");
    pub static ref NVIDIA_API_KEY: String = load_api_key("NVIDIA_API_KEY");
    pub static ref OPENROUTER_API_KEY: String = load_api_key("OPENROUTER_API_KEY");
    pub static ref ASSEMBLY_AI_KEY: String = load_api_key("ASSEMBLY_AI_KEY");
    pub static ref SCREENSHOT_API_KEY: String = load_api_key("SCREENSHOT_API_KEY");
    // Shared HTTP client for connection pooling - major performance boost
    pub static ref HTTP_CLIENT: Client = Client::builder()
        .timeout(std::time::Duration::from_secs(180))  // 3 minutes for large image analysis
        .connect_timeout(std::time::Duration::from_secs(15))  // Connection timeout
        .pool_max_idle_per_host(5)
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .build()
        .expect("Failed to create HTTP client");
}

// Available AI models (OpenAI removed)
pub enum AIModel {
    OpenRouterGemini,
    OpenRouterMistral,
    OpenRouterLlama33,
    OpenRouterQwen36Plus,
    OpenRouterDevstral2,
    OpenRouterDeepSeekR1,
    OpenRouterNemotron3,
    ClaudeOpus,
    GroqLlama3,
    LocalOllama,
}

impl AIModel {
    pub fn name(&self) -> &'static str {
        match self {
            AIModel::OpenRouterGemini => "OpenRouter Gemini 2.5 Flash",
            AIModel::OpenRouterMistral => "OpenRouter Mistral 7B Instruct",
            AIModel::OpenRouterLlama33 => "OpenRouter Llama 3.3 70B",
            AIModel::OpenRouterQwen36Plus => "OpenRouter Qwen 3.6 Plus",
            AIModel::OpenRouterDevstral2 => "OpenRouter Devstral 2",
            AIModel::OpenRouterDeepSeekR1 => "OpenRouter DeepSeek R1",
            AIModel::OpenRouterNemotron3 => "OpenRouter Nemotron 3 Super",
            AIModel::ClaudeOpus => "Claude Opus",
            AIModel::GroqLlama3 => "Groq LLaMA 3.3",
            AIModel::LocalOllama => "Local Ollama (Phi)",
        }
    }
    
    pub fn api_model_id(&self) -> &'static str {
        match self {
            AIModel::OpenRouterGemini => "google/gemini-2.5-flash",
            AIModel::OpenRouterMistral => "mistralai/mistral-7b-instruct-v0.1",
            AIModel::OpenRouterLlama33 => "meta-llama/llama-3.3-70b-instruct:nitro",
            AIModel::OpenRouterQwen36Plus => "qwen/qwen3.6-plus:nitro",
            AIModel::OpenRouterDevstral2 => "mistralai/devstral-2512:nitro",
            AIModel::OpenRouterDeepSeekR1 => "deepseek/deepseek-r1:nitro",
            AIModel::OpenRouterNemotron3 => "nvidia/nemotron-3-super-120b-a12b:nitro",
            AIModel::ClaudeOpus => "claude-3-opus-20240229",
            AIModel::GroqLlama3 => "llama-3.3-70b-versatile",
            AIModel::LocalOllama => "phi",
        }
    }
    
    pub fn is_gemini(&self) -> bool {
        matches!(self, AIModel::OpenRouterGemini)
    }

    pub fn supports_images(&self) -> bool {
        matches!(self, AIModel::OpenRouterGemini)
    }
    
    pub fn from_index(index: usize) -> Self {
        match index % 10 {
             0 => AIModel::OpenRouterGemini,
             1 => AIModel::OpenRouterMistral,
             2 => AIModel::OpenRouterLlama33,
             3 => AIModel::OpenRouterQwen36Plus,
             4 => AIModel::OpenRouterDevstral2,
             5 => AIModel::OpenRouterDeepSeekR1,
             6 => AIModel::OpenRouterNemotron3,
             7 => AIModel::GroqLlama3,
             8 => AIModel::ClaudeOpus,
             _ => AIModel::LocalOllama,
        }
    }
}

static CURRENT_MODEL_INDEX: AtomicUsize = AtomicUsize::new(0);

pub fn get_current_model() -> AIModel {
    AIModel::from_index(CURRENT_MODEL_INDEX.load(Ordering::SeqCst))
}

pub fn get_current_model_name() -> String {
    get_current_model().name().to_string()
}

pub fn cycle_model() -> AIModel {
    let current = CURRENT_MODEL_INDEX.load(Ordering::SeqCst);
    let next = (current + 1) % 10;
    CURRENT_MODEL_INDEX.store(next, Ordering::SeqCst);
    let model = AIModel::from_index(next);
    crate::log_info!("Switched to model: {}", model.name());
    model
}

// System prompt (Kept same)
const SYSTEM_PROMPT: &str = r#"You are an expert interview problem-solving assistant. Input is always an IMAGE (screenshots of questions/code/SQL/errors/whiteboard). Help candidates crack interviews with clear, structured answers in simple English.

RULES:
- NEVER repeat sentences or paragraphs. Keep your explanation strictly forward-moving, concise, and non-repetitive. Avoid looping back to points you have already made.
- Carefully read & understand the full image before answering. Extract problem/code/errors from it; don't assume missing details.
but first check weather it is mcq or not and 
if question is mcq type then just give me only correct option nothing else just correct answer
- Explain calmly in interview-friendly manner. Never skip steps. Prefer correctness & clarity over brevity.

🔹 DSA / CODING RESPONSE FORMAT (STRICT)

Language

Auto-detect from code in image.

If not clear → use C++ (Unless the USER DEFINED RULES block specifically overrides this with another language).

Intuition

Explain thinking process.

Mention patterns, DS, algorithm category.

Approaches (in this order)

A) Brute Force

1)Idea

2)Why it works

3)Time Complexity (with reasoning, not just Big-O)

4)Space Complexity (with reasoning)

B) Optimal

1)Step-by-step logic

2)Why it is better than brute force

3)Time Complexity (with reasoning)

4)Space Complexity (with reasoning)

Dry Run

1)Step-by-step execution using example input

2)Show variable changes clearly

Code Section (MANDATORY RULES)

1) ONLY write the Optimal Code. Do NOT write the code for the Brute Force approach.

2) The Optimal Code must exactly match the previously explained Optimal approach.

3) Add detailed inline comments to EVERY SINGLE LINE of code to explain exactly what it does.

4) Keep the code clean, well-indented, and interview-ready.

SQL:
1. Explain problem & data.
2. Write basic query, then optimized query (if possible).
3. Explain what each does, why it works, why optimized is better.

THEORY (OS/DBMS/CN/OOPS etc.):
- Clear, deep, structured explanation with headings, bullets, examples. Interview-oriented & revision-friendly.

ERROR-BASED (Code/SQL):
1. Identify & explain what the error is and why it occurs.
2. Explain the fix.
3. Provide corrected code/query with error fully resolved.

WEB DEV / MACHINE CODING:
1. Explain problem & requirements.
2. Break into logical steps; explain architecture, components, flow.
3. Write clean, working code with explanation. Follow best practices.

ALWAYS: Structure answers. Explain before coding. Optimize when possible. Assume live interview scenario. Help candidate think, not just copy."#;




const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:89.0) Gecko/20100101 Firefox/89.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15"
];

pub fn init() {
    dotenv::dotenv().ok();
    println!("AI module initialized with model: {}", get_current_model_name());
}

// Helper to accumulate sentence buffer and "speak"
fn process_text_chunk_for_voice(chunk: &str, buffer: &mut String) {
    buffer.push_str(chunk);
    
    // Check for sentence delimiters
    let delimiters = ['.', '?', '!', '\n'];
    
    let mut last_processed_index = 0;
    let chars: Vec<char> = buffer.chars().collect(); // Naive char iteration
    let mut found_delimiter = false;

    // We scan to find the last valid delimiter to split safe sentences
    // This is a simple heuristic.
    for (i, &c) in chars.iter().enumerate() {
        if delimiters.contains(&c) {
            // Found a sentence end.
            // Check if it's not part of a number like 3.14 (basic check)
             let is_number = if c == '.' && i > 0 && i + 1 < chars.len() {
                chars[i-1].is_numeric() && chars[i+1].is_numeric()
            } else {
                false
            };

            if !is_number {
                // It is a real delimiter
                let sentence: String = chars[last_processed_index..=i].iter().collect();
                if !sentence.trim().is_empty() {
                    // MOCK TTS CALL
                    crate::log_info!("[VOICE PIPELINE] TTS Speaking: {}", sentence.trim());
                    // Also print to stdout for verification
                    println!("[VOICE PIPELINE] TTS Speaking: {}", sentence.trim());
                }
                last_processed_index = i + 1;
                found_delimiter = true;
            }
        }
    }

    if found_delimiter {
        // Remove processed part from buffer
        // Reconstruct string from remaining chars
        let remaining: String = chars[last_processed_index..].iter().collect();
        *buffer = remaining;
    }
}

// Async Implementation for Gemini - OPTIMIZED with shared client
fn get_screenshot_system_prompt() -> String {
    let user_context = crate::config::user_profile::get_ai_context();
    format!(
"CRITICAL USER DEFINED RULES:
You MUST strictly follow any custom rules or preferences provided in the block below:
---
{}
---

{}", user_context, SYSTEM_PROMPT)
}

async fn call_gemini_api_stream(prompt: &str, image_paths: Vec<PathBuf>, model: AIModel, callback: impl Fn(String) + Send + Sync + 'static) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Use shared HTTP client for connection pooling
    let client = &*HTTP_CLIENT;
    
    crate::log_info!("Starting Gemini API call for {} images", image_paths.len());
    callback("⏳ Connecting to Gemini...".to_string());
    let start = std::time::Instant::now();

    let base64_images = prepare_images_base64(&image_paths);
    crate::log_info!("Image prep done in {:?}", start.elapsed());

    if base64_images.is_empty() {
        return Err("No valid images to process".into());
    }

    let mut parts = Vec::new();
    for img in &base64_images {
        parts.push(json!({
            "inline_data": {
                "mime_type": "image/jpeg",  // Changed to JPEG since we resize to JPEG
                "data": img
            }
        }));
    }

    // Get personalized context from user profile
    let user_context = get_ai_context();
    
    // Build user prompt (Context + Specific Prompt)
    let user_prompt = if user_context.is_empty() {
        format!("{}\n\nSolve this.", prompt)
    } else {
        format!("User Context: {}\n\nRequest: {}\n\nSolve this.", user_context, prompt)
    };

    parts.push(json!({
        "text": user_prompt
    }));

    let body = json!({
        "systemInstruction": {
            "parts": [
                { "text": get_screenshot_system_prompt() }
            ]
        },
        "contents": [{
            "parts": parts
        }],
        "generationConfig": {
            "maxOutputTokens": 16384,
            "temperature": 0.1
        }
    });

    let model_id = model.api_model_id();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?key={}",
        model_id,
        SCREENSHOT_API_KEY.as_str()
    );

    let ua_index = rand::random::<usize>() % USER_AGENTS.len();
    let user_agent = USER_AGENTS[ua_index];
    
    crate::log_info!("Sending request to Gemini API...");
    let request_start = std::time::Instant::now();

    let response = client
        .post(&url)
        .header("content-type", "application/json")
        .header("user-agent", user_agent)
        .json(&body)
        .send()
        .await?;
    
    let status = response.status();
    crate::log_info!("Got response in {:?}, status: {}", request_start.elapsed(), status);
    
    if !status.is_success() {
        let err_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Gemini API Error ({}): {}", status, err_text).into());
    }
    
    let mut response_stream = response.bytes_stream();

    let mut full_text = String::new();
    let mut json_buffer = String::new();
    let mut voice_buffer = String::new();
    let mut chunk_count = 0;
    let mut decoded_buffer = Vec::new();

    while let Some(chunk_result) = response_stream.next().await {
        let chunk = chunk_result?;
        chunk_count += 1;
        if chunk_count <= 3 {
            crate::log_info!("Chunk #{}: {} bytes", chunk_count, chunk.len());
        }
        
        // 🎯 UTF-8 FIX: Accumulate raw bytes to prevent corruption of split multi-byte characters
        decoded_buffer.extend_from_slice(&chunk);
        
        // 🎯 UTF-8 SAFE DECODING: Only process what is valid UTF-8
        let (json_str, _ ) = match std::str::from_utf8(&decoded_buffer) {
            Ok(s) => (s, decoded_buffer.len()),
            Err(e) => {
                let valid_up_to = e.valid_up_to();
                // If 0, we have no valid UTF-8 yet
                if valid_up_to == 0 { ( "", 0 ) } 
                else { (std::str::from_utf8(&decoded_buffer[..valid_up_to]).unwrap(), valid_up_to) }
            }
        };
        
        if json_str.is_empty() { continue; }
        json_buffer = json_str.to_string();

        // OPTIMIZED JSON parsing: Look for complete objects by finding matching braces
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
            
            // Extract and parse the JSON object
            let potential_json = &json_buffer[start_pos..end_pos];
            
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(potential_json) {
                // 🎯 MULTI-PART FIX: Iterate over ALL parts in the response
                if let Some(candidates) = val.get("candidates").and_then(|c| c.as_array()) {
                    for cand in candidates {
                        if let Some(parts) = cand.pointer("/content/parts").and_then(|p| p.as_array()) {
                            for part in parts {
                                if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                                    full_text.push_str(t);
                                    crate::log_info!("AI part: {} chars, total: {}", t.len(), full_text.len());
                                    // Use append_ai_response style immediately
                                    callback(full_text.clone());
                                    process_text_chunk_for_voice(t, &mut voice_buffer);
                                }
                            }
                        }
                    }
                }
            }
            
            // Drain from byte buffer based on the character boundary we just processed
            let byte_len = json_buffer[..end_pos].len();
            decoded_buffer.drain(..byte_len);
            json_buffer = json_buffer[end_pos..].to_string();
        }
    }
    
    // Final update to ensure the UI paints the complete response (bypass throttling)
    crate::overlay::stealth::force_ai_response_update(&full_text);
    
    crate::log_info!("Streaming complete. Total response: {} chars", full_text.len());
    Ok(full_text)
}


// Async Implementation for Gemini Audio - OPTIMIZED with shared client
async fn ask_ai_with_audio_async(audio_bytes: Vec<u8>, callback: impl Fn(String) + Send + Sync + 'static) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Use shared HTTP client for connection pooling
    let client = &*HTTP_CLIENT;
        
    use base64::{Engine as _, engine::general_purpose};
    let base64_audio = general_purpose::STANDARD.encode(&audio_bytes);
    
    let model = get_current_model();
    println!("DEBUG: ask_ai_with_audio_async started. Model: {}", model.name());
    
    let model_to_use = if model.is_gemini() {
        model.api_model_id()
    } else {
        "gemini-1.5-pro"
    };

    let body = json!({
        "contents": [{
            "parts": [
                { "text": "Listen to this audio carefully and answer the question being asked directly." },
                {
                    "inline_data": {
                        "mime_type": "audio/wav",
                        "data": base64_audio
                    }
                }
            ]
        }],
        "generationConfig": {
            "maxOutputTokens": 16384,
            "temperature": 0.1
        }
    });

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?key={}",
        model_to_use,
        GOOGLE_API_KEY.as_str()
    );
    
    let mut response_stream = client
        .post(&url)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?
        .bytes_stream();

    let mut full_text = String::new();
    let mut json_buffer = String::new();
    let mut voice_buffer = String::new();

    // OPTIMIZED: Same parsing logic as Image stream
    let mut decoded_buffer = Vec::new();
    while let Some(chunk_result) = response_stream.next().await {
        let chunk = chunk_result?;
        decoded_buffer.extend_from_slice(&chunk);
        
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
                if let Some(candidates) = val.get("candidates").and_then(|c| c.as_array()) {
                    for cand in candidates {
                        if let Some(parts) = cand.pointer("/content/parts").and_then(|p| p.as_array()) {
                            for part in parts {
                                if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                                    full_text.push_str(t);
                                    callback(full_text.clone());
                                    process_text_chunk_for_voice(t, &mut voice_buffer);
                                }
                            }
                        }
                    }
                }
            }
            let byte_len = json_buffer[..end_pos].len();
            decoded_buffer.drain(..byte_len);
            json_buffer = json_buffer[end_pos..].to_string();
        }
    }
    
    Ok(full_text)
}


// Public wrappers - NOW NON-BLOCKING with spawn
// The callback is called as chunks arrive, enabling real-time UI updates
pub fn ask_ai_with_images(question: &str, image_paths: Vec<PathBuf>, callback: impl Fn(String) + Send + Sync + 'static) -> String {
    // Guard: Prevent multiple simultaneous queries
    if AI_QUERY_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        callback("⚠️ AI query already in progress...".to_string());
        return "⚠️ Query in progress".to_string();
    }
    
    let model = get_current_model();
    crate::log_info!("Sending {} images to {} AI", image_paths.len(), model.name());

    if model.is_gemini() {
        // Clone data for the spawned thread
        let question = question.to_string();
        let callback = std::sync::Arc::new(callback);
        let callback_clone = callback.clone();
        
        // Use std::thread + block_on instead of Tokio spawn
        // This ensures the async code actually runs in DLL context!
        std::thread::spawn(move || {
            crate::log_info!("AI thread started, calling Gemini API...");
            
            // Run the async code using Tokio's block_on
            let result = crate::TOKIO_RT.block_on(async move {
                call_gemini_api_stream(&question, image_paths, model, move |text| {
                    callback_clone(text);
                }).await
            });
            
            match result {
                Ok(final_response) => {
                    crate::log_info!("Streaming complete. Final length: {}", final_response.len());
                }
                Err(e) => {
                    let err_msg = format!("❌ Error: {}", e);
                    crate::log_error!("{}", err_msg);
                    callback(err_msg);
                }
            }
            // Release the guard when done
            AI_QUERY_IN_PROGRESS.store(false, Ordering::SeqCst);
            crate::log_info!("AI thread completed");
        });
        
        // Return immediately - UI will update via callback
        "⏳ Streaming...".to_string()
    } else {
        // Fallback for Claude (Sync) - still blocking but Claude doesn't support streaming well
        let res = call_claude_api_with_images(question, image_paths).unwrap_or_else(|e| format!("Error: {}", e));
        callback(res.clone());
        AI_QUERY_IN_PROGRESS.store(false, Ordering::SeqCst); // Release guard
        res
    }
}

pub fn ask_ai(question: &str) -> String {
    let screenshot_path = std::fs::read_dir(".")
        .ok()
        .and_then(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .contains("tmp_") 
                        && e.file_name().to_string_lossy().ends_with(".dat")
                })
                .max_by_key(|e| e.metadata().ok().and_then(|m| m.modified().ok()))
                .map(|e| e.path())
        });
    
    if let Some(path) = screenshot_path {
        ask_ai_with_images(question, vec![path], |_| {})
    } else {
        "No screenshots found".to_string()
    }
}

pub fn query_screenshot(path: &PathBuf) -> String {
    ask_ai_with_images("What do you see in this image? Provide a detailed description.", vec![path.clone()], |_| {})
}

pub fn ask_ai_with_audio(audio_bytes: Vec<u8>, callback: impl Fn(String) + Send + Sync + 'static) -> Result<String, Box<dyn std::error::Error>> {
    let callback = std::sync::Arc::new(callback);
    let callback_clone = callback.clone();
    
    // Use std::thread + block_on instead of Tokio spawn (DLL context fix)
    std::thread::spawn(move || {
        crate::log_info!("Audio AI thread started...");
        
        let result = crate::TOKIO_RT.block_on(async move {
            ask_ai_with_audio_async(audio_bytes, move |text| {
                callback_clone(text);
            }).await
        });
        
        match result {
            Ok(final_response) => {
                crate::log_info!("Audio streaming complete. Final length: {}", final_response.len());
            }
            Err(e) => {
                let err_msg = format!("Audio Error: {}", e);
                crate::log_error!("{}", err_msg);
                callback(err_msg);
            }
        }
    });
    
    // Return immediately - UI will update via callback
    Ok("🎤 Processing audio...".to_string())
}

// Helpers
fn prepare_images_base64(image_paths: &[PathBuf]) -> Vec<String> {
    use base64::{Engine as _, engine::general_purpose};
    use image::io::Reader as ImageReader;
    use std::io::Cursor;
    
    let mut base64_images = vec![];
    let start = std::time::Instant::now();
    
    for (index, path) in image_paths.iter().enumerate() {
        if path.exists() {
            match std::fs::read(path) {
                Ok(png_data) => {
                    // Direct read - no decryption needed
                    crate::log_info!("Image {} loaded: {} KB", index + 1, png_data.len() / 1024);
                    
                    // CRITICAL: Resize image to max 1280px width for faster AI processing
                    let resized_data = match ImageReader::new(Cursor::new(&png_data))
                        .with_guessed_format()
                        .and_then(|r| r.decode().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)))
                    {
                        Ok(img) => {
                            let (w, h) = (img.width(), img.height());
                            // Resize to max 1568px width (Standard for Gemini)
                            let resized = if w > 1568 {
                                let new_height = (h as f32 * 1568.0 / w as f32) as u32;
                                img.resize(1568, new_height, image::imageops::FilterType::Triangle)
                            } else {
                                img
                            };
                            
                            // Encode as JPEG with lower quality for speed
                            let mut jpeg_buffer = Vec::new();
                            {
                                let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_buffer, 90);
                                if encoder.encode_image(&resized).is_ok() {
                                    crate::log_info!("Resized {}x{} -> {}x{}, JPEG: {} KB", 
                                        w, h, resized.width(), resized.height(), jpeg_buffer.len() / 1024);
                                }
                            }
                            if !jpeg_buffer.is_empty() {
                                jpeg_buffer
                            } else {
                                png_data // Fallback to original
                            }
                        }
                        Err(_) => png_data // Fallback to original if resize fails
                    };
                    
                    let base64_image = general_purpose::STANDARD.encode(&resized_data);
                    base64_images.push(base64_image);
                }
                Err(e) => crate::log_error!("Failed to read image {}: {}", index + 1, e),
            }
        }
    }
    
    crate::log_info!("Image preparation took {:?}", start.elapsed());
    base64_images
}

fn decrypt_screenshot(encrypted_data: &[u8]) -> Vec<u8> {
    const ENCRYPTION_KEY: [u8; 32] = [0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6,
                                      0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf, 0x4f, 0x3c,
                                      0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6,
                                      0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf, 0x4f, 0x3c];
    encrypted_data.iter()
        .enumerate()
        .map(|(i, &byte)| byte ^ ENCRYPTION_KEY[i % ENCRYPTION_KEY.len()])
        .collect()
}

// Original Claude Sync Implementation (Unchanged mostly)
fn call_claude_api_with_images(_prompt: &str, image_paths: Vec<PathBuf>) -> Result<String, Box<dyn std::error::Error>> {
    use reqwest::blocking::Client;
    use serde_json::json;
    use std::time::Duration;
    
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;
    
    let base64_images = prepare_images_base64(&image_paths);
    
    if base64_images.is_empty() { return Err("No valid images".into()); }
    
    let mut content_parts: Vec<serde_json::Value> = base64_images.iter().map(|img| {
        json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": "image/png",
                "data": img
            }
        })
    }).collect();
    
    content_parts.push(json!({ "type": "text", "text": "Solve this." }));
    
    let body = json!({
        "model": "claude-3-opus-20240229",
        "max_tokens": 4096,
        "system": get_screenshot_system_prompt(),
        "messages": [{
            "role": "user",
            "content": content_parts
        }]
    });
    
    let ua_index = rand::random::<usize>() % USER_AGENTS.len();
    let user_agent = USER_AGENTS[ua_index];
    
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", CLAUDE_API_KEY.as_str())
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .header("user-agent", user_agent)
        .json(&body)
        .send()?;
    
    let text = response.text()?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    
    if let Some(content) = json["content"][0]["text"].as_str() {
        Ok(content.to_string())
    } else {
        Err(format!("Error: {}", text).into())
    }
}
