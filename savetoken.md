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
    Gemini3Pro,
    Gemini3Flash,
    Gemini25Pro,
    Gemini25Flash,
    Gemini20Flash,
    Gemini15Pro,
    Gemini15Flash,
    ClaudeOpus,
}

impl AIModel {
    pub fn name(&self) -> &'static str {
        match self {
            AIModel::Gemini3Pro => "Gemini 3.0 Pro",
            AIModel::Gemini3Flash => "Gemini 3.0 Flash",
            AIModel::Gemini25Pro => "Gemini 2.5 Pro",
            AIModel::Gemini25Flash => "Gemini 2.5 Flash",
            AIModel::Gemini20Flash => "Gemini 2.0 Flash",
            AIModel::Gemini15Pro => "Gemini 1.5 Pro",
            AIModel::Gemini15Flash => "Gemini 1.5 Flash",
            AIModel::ClaudeOpus => "Claude Opus",
        }
    }
    
    pub fn api_model_id(&self) -> &'static str {
        match self {
            AIModel::Gemini3Pro => "gemini-3-pro-preview",
            AIModel::Gemini3Flash => "gemini-3-flash-preview",
            AIModel::Gemini25Pro => "gemini-2.5-pro",
            AIModel::Gemini25Flash => "gemini-2.5-flash",
            AIModel::Gemini20Flash => "gemini-2.0-flash",
            AIModel::Gemini15Pro => "gemini-1.5-pro",
            AIModel::Gemini15Flash => "gemini-1.5-flash",
            AIModel::ClaudeOpus => "claude-3-opus-20240229",
        }
    }
    
    pub fn is_gemini(&self) -> bool {
        !matches!(self, AIModel::ClaudeOpus)
    }
    
    pub fn from_index(index: usize) -> Self {
        match index % 8 {
             0 => AIModel::Gemini20Flash,
             1 => AIModel::Gemini25Pro,
             2 => AIModel::Gemini25Flash,
             3 => AIModel::Gemini3Pro,
             4 => AIModel::Gemini3Flash,
             5 => AIModel::Gemini15Pro,
             6 => AIModel::Gemini15Flash,
             _ => AIModel::ClaudeOpus,
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
    let next = (current + 1) % 8;
    CURRENT_MODEL_INDEX.store(next, Ordering::SeqCst);
    let model = AIModel::from_index(next);
    crate::log_info!("Switched to model: {}", model.name());
    model
}

// System prompt (Kept same)
const SYSTEM_PROMPT: &str = r#"You are an expert interview problem-solving assistant. Input is always an IMAGE (screenshots of questions/code/SQL/errors/whiteboard). Help candidates crack interviews with clear, structured answers in simple English.

RULES:
- Carefully read & understand the full image before answering. Extract problem/code/errors from it; don't assume missing details.
- Explain calmly in interview-friendly manner. Never skip steps. Prefer correctness & clarity over brevity.

🔹 DSA / CODING RESPONSE FORMAT (STRICT)

Language

Auto-detect from code in image.

If not clear → use C++.

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

1)First write Brute Force Code

2)Then write Optimal Code

3)Both implementations must be fully written

4)Both must be clean, commented, interview-ready

5)Both must exactly match the previously explained approaches

6)Do NOT skip brute-force implementation

7)Do NOT merge both approaches into one function

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
                { "text": SYSTEM_PROMPT }
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

    while let Some(chunk_result) = response_stream.next().await {
        let chunk = chunk_result?;
        chunk_count += 1;
        if chunk_count <= 3 {
            crate::log_info!("Chunk #{}: {} bytes", chunk_count, chunk.len());
        }
        let chunk_str = String::from_utf8_lossy(&chunk);
        
        json_buffer.push_str(&chunk_str);

        // OPTIMIZED JSON parsing: Look for complete objects by finding matching braces
        // Process all complete JSON objects in the buffer
        loop {
            // Find the first '{' in the buffer
            let start_pos = match json_buffer.find('{') {
                Some(pos) => pos,
                None => break, // No object start found
            };
            
            // Count braces to find matching '}'
            let mut brace_count = 0;
            let mut in_string = false;
            let mut escape = false;
            let mut end_pos = None;
            
            for (i, c) in json_buffer[start_pos..].char_indices() {
                if escape {
                    escape = false;
                    continue;
                }
                if c == '\\' {
                    escape = true;
                    continue;
                }
                if c == '"' {
                    in_string = !in_string;
                    continue;
                }
                if !in_string {
                    if c == '{' {
                        brace_count += 1;
                    } else if c == '}' {
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
                None => break, // Incomplete object, wait for more data
            };
            
            // Extract and parse the JSON object
            let potential_json = &json_buffer[start_pos..end_pos];
            
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(potential_json) {
                // 🎯 MULTI-PART FIX: Iterate over ALL parts
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
            
            // Remove processed portion from buffer
            json_buffer = json_buffer[end_pos..].to_string();
        }
    }
    
    // Process any remaining specific text in buffer if any (cleanup)
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
    while let Some(chunk_result) = response_stream.next().await {
        let chunk = chunk_result?;
        let chunk_str = String::from_utf8_lossy(&chunk);
        
        json_buffer.push_str(&chunk_str);

        // Process all complete JSON objects in buffer
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
        "max_tokens": 2048,
        "system": SYSTEM_PROMPT,
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








use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleA,
        UI::WindowsAndMessaging::*,
        UI::Input::KeyboardAndMouse::{
            RegisterHotKey, MOD_ALT, MOD_SHIFT, 
            VK_LEFT, VK_RIGHT, VK_UP, VK_DOWN,
            VK_ADD, VK_SUBTRACT, VK_OEM_PLUS, VK_OEM_MINUS
        },
    },
};

// For rounded corners
use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Mutex;
use std::os::windows::process::CommandExt; // For creation_flags
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

// Import SetForegroundWindow
use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

// =============================================================================
// 🎹 HOTKEY ID CONSTANTS
// =============================================================================
// Use named constants instead of magic numbers for maintainability.
// Format: HOTKEY_{MODIFIER}_{KEY} = {id}
// 
// ID Ranges:
//   1000-1099: Alt+Shift combinations (legacy)
//   1100-1199: Shift+only (priority actions)
//   1200-1299: Screenshot workflow
//   1300-1399: UI control
// =============================================================================

// Priority Actions (Shift + Key)
const HOTKEY_SHIFT_Q: usize = 1100;     // Send transcription to AI NOW
const HOTKEY_SHIFT_A: usize = 1101;     // Clear ALL

// Screenshot Workflow (Shift + Key)
const HOTKEY_SHIFT_S: usize = 1200;     // Capture screenshot
const HOTKEY_SHIFT_D: usize = 1201;     // Delete all screenshots
const HOTKEY_SHIFT_F: usize = 1202;     // Send/Flash to AI
const HOTKEY_SHIFT_M: usize = 1203;     // Cycle AI model

// UI Control (Shift + Key)
const HOTKEY_SHIFT_U: usize = 1300;     // Scroll UP
const HOTKEY_SHIFT_N: usize = 1301;     // Scroll DOWN
const HOTKEY_SHIFT_T: usize = 1302;     // Toggle visibility
const HOTKEY_SHIFT_P: usize = 1303;     // Toggle (legacy)

// Alt+Shift Combinations
const HOTKEY_ALT_SHIFT_TOGGLE: usize = 1001;    // Legacy toggle
const HOTKEY_ALT_SHIFT_TEST: usize = 1002;      // Test overlay
const HOTKEY_ALT_SHIFT_S: usize = 1003;         // Screenshot (alt)
const HOTKEY_ALT_SHIFT_A: usize = 1004;         // Clear (alt)
const HOTKEY_ALT_SHIFT_D: usize = 1005;         // Delete (alt)
const HOTKEY_ALT_SHIFT_LEFT: usize = 1006;      // Move left
const HOTKEY_ALT_SHIFT_RIGHT: usize = 1007;     // Move right
const HOTKEY_ALT_SHIFT_UP: usize = 1008;        // Move up
const HOTKEY_ALT_SHIFT_DOWN: usize = 1009;      // Move down
const HOTKEY_ALT_SHIFT_W: usize = 1010;         // Increase height
const HOTKEY_ALT_SHIFT_X: usize = 1011;         // Decrease height
const HOTKEY_ALT_SHIFT_PLUS: usize = 1012;      // Increase width (numpad)
const HOTKEY_ALT_SHIFT_MINUS: usize = 1013;     // Decrease width (numpad)
const HOTKEY_ALT_SHIFT_OEM_PLUS: usize = 1014;  // Increase width
const HOTKEY_ALT_SHIFT_OEM_MINUS: usize = 1015; // Decrease width
const HOTKEY_ALT_SHIFT_K: usize = 1016;         // Kill/stop
const HOTKEY_ALT_SHIFT_I: usize = 1017;         // Input/AutoType
const HOTKEY_ALT_SHIFT_B: usize = 1018;         // Toggle auto-bracket
const HOTKEY_ALT_SHIFT_M: usize = 1019;         // Cycle AI model
const HOTKEY_ALT_SHIFT_V: usize = 1020;         // Voice mode
const HOTKEY_ALT_SHIFT_C: usize = 1021;         // Close live session
const HOTKEY_ALT_SHIFT_Z: usize = 1022;         // Manual clear

// Timer ID
const TIMER_HEARTBEAT: usize = 999;

// =============================================================================
// Window Display Affinity constants
// =============================================================================
const WDA_NONE: WINDOW_DISPLAY_AFFINITY = WINDOW_DISPLAY_AFFINITY(0);
const WDA_MONITOR: WINDOW_DISPLAY_AFFINITY = WINDOW_DISPLAY_AFFINITY(1);
const WDA_EXCLUDEFROMCAPTURE: WINDOW_DISPLAY_AFFINITY = WINDOW_DISPLAY_AFFINITY(0x11);

static OVERLAY_HWND: AtomicIsize = AtomicIsize::new(0);
static OVERLAY_VISIBLE: AtomicBool = AtomicBool::new(true);
static SCROLL_OFFSET: AtomicIsize = AtomicIsize::new(0);
static CACHED_FONT: AtomicIsize = AtomicIsize::new(0);  // Cached font handle for performance
static LAST_TEXT_HEIGHT: AtomicIsize = AtomicIsize::new(0);  // Cache text height calculation

lazy_static::lazy_static! {
    static ref OVERLAY_TEXT: Mutex<String> = Mutex::new(String::from("📡 SM Active\n\n🔒 Stealth: ON\n👁️ You only"));
    static ref LAST_AI_RESPONSE: Mutex<String> = Mutex::new(String::new());
    static ref LIVE_TRANSCRIPTION: Mutex<String> = Mutex::new(String::new());
    static ref STATUS_MESSAGE: Mutex<String> = Mutex::new(String::new());
    static ref STATUS_EXPIRY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    static ref GDI_CACHE: Mutex<Option<GdiCache>> = Mutex::new(None);
}

struct GdiCache {
    font_main: HFONT,
    font_bold: HFONT,
    font_title: HFONT,
    font_small: HFONT,
    br_bg: HBRUSH,
    br_card: HBRUSH,
    br_card_ai: HBRUSH,
    br_border: HBRUSH,
    br_border_glow: HBRUSH,
    br_accent: HBRUSH,
    gradient_brushes: [HBRUSH; 64],
    left_accent_brushes: [HBRUSH; 2],
}

impl GdiCache {
    unsafe fn new() -> Self {
        // 🎨 PREMIUM FONTS - Segoe UI for clean, modern look (High Clarity)
        let font_segoe = PCSTR("Segoe UI\0".as_ptr());
        
        // Increased font size to 30px for main text, 400 weight (Normal) for thinner, crisp look
        let font_main = CreateFontA(30, 0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET.0 as u32, OUT_DEFAULT_PRECIS.0 as u32, CLIP_DEFAULT_PRECIS.0 as u32, ANTIALIASED_QUALITY.0 as u32, (DEFAULT_PITCH.0 | FF_SWISS.0) as u32, font_segoe);
        let font_bold = CreateFontA(30, 0, 0, 0, 600, 0, 0, 0, DEFAULT_CHARSET.0 as u32, OUT_DEFAULT_PRECIS.0 as u32, CLIP_DEFAULT_PRECIS.0 as u32, ANTIALIASED_QUALITY.0 as u32, (DEFAULT_PITCH.0 | FF_SWISS.0) as u32, font_segoe);
        let font_title = CreateFontA(26, 0, 0, 0, 600, 0, 0, 0, DEFAULT_CHARSET.0 as u32, OUT_DEFAULT_PRECIS.0 as u32, CLIP_DEFAULT_PRECIS.0 as u32, ANTIALIASED_QUALITY.0 as u32, (DEFAULT_PITCH.0 | FF_SWISS.0) as u32, font_segoe);
        let font_small = CreateFontA(24, 0, 0, 0, 300, 0, 0, 0, DEFAULT_CHARSET.0 as u32, OUT_DEFAULT_PRECIS.0 as u32, CLIP_DEFAULT_PRECIS.0 as u32, ANTIALIASED_QUALITY.0 as u32, (DEFAULT_PITCH.0 | FF_SWISS.0) as u32, font_segoe);
        
        // 🎨 WATER GLASS THEME - Lighter greys for transparency
        // BGR format for Windows
        let color_bg_main    = 0x00050505;  // Almost black (Deep Deep Grey)
        let color_bg_card    = 0x00101010;  // Deep charcoal
        let color_bg_card_ai = 0x00080808;  // Near black
        let color_border     = 0x00666666;  // Glass edge
        let color_border_glow= 0x00999999;  // Bright edge highlight
        let color_accent     = 0x00FFFFFF;  // Pure White
        
        let br_bg = CreateSolidBrush(COLORREF(color_bg_main));
        let br_card = CreateSolidBrush(COLORREF(color_bg_card));
        let br_card_ai = CreateSolidBrush(COLORREF(color_bg_card_ai));
        let br_border = CreateSolidBrush(COLORREF(color_border));
        let br_border_glow = CreateSolidBrush(COLORREF(color_border_glow));
        let br_accent = CreateSolidBrush(COLORREF(color_accent));

        // ⚪ NEUTRAL GRADIENTS - Pure white intensities
        let mut gradient_brushes = [HBRUSH::default(); 64];
        for i in 0..64 {
            let shade = 220 + (i / 2) as u8;
            let color = (shade as u32) | ((shade as u32) << 8) | ((shade as u32) << 16);
            gradient_brushes[i as usize] = CreateSolidBrush(COLORREF(color));
        }
        
        // Left accent bars for cards - Pure Neutral Clear
        let left_accent_brushes = [
            CreateSolidBrush(COLORREF(0x00DDDDDD)),  // Light silver
            CreateSolidBrush(COLORREF(0x00FFFFFF)),  // Pure white
        ];

        Self { font_main, font_bold, font_title, font_small, br_bg, br_card, br_card_ai, br_border, br_border_glow, br_accent, gradient_brushes, left_accent_brushes }
    }

    unsafe fn cleanup(&self) {
        // Delete font objects
        let _ = DeleteObject(self.font_main);
        let _ = DeleteObject(self.font_bold);
        let _ = DeleteObject(self.font_title);
        let _ = DeleteObject(self.font_small);
        
        // Delete brush objects
        let _ = DeleteObject(self.br_bg);
        let _ = DeleteObject(self.br_card);
        let _ = DeleteObject(self.br_card_ai);
        let _ = DeleteObject(self.br_border);
        let _ = DeleteObject(self.br_border_glow);
        let _ = DeleteObject(self.br_accent);
        
        // Delete gradient brushes
        for br in self.gradient_brushes {
            let _ = DeleteObject(br);
        }
        
        // Delete accent brushes
        for br in self.left_accent_brushes {
            let _ = DeleteObject(br);
        }
        
        crate::log_info!("[GDI] All cached GDI objects cleaned up");
    }
}

// Implement Drop for automatic cleanup if GdiCache goes out of scope unexpectedly
impl Drop for GdiCache {
    fn drop(&mut self) {
        unsafe { self.cleanup(); }
    }
}

/// Initialize the overlay (BLOCKS until overlay exits)
/// This runs the Windows message loop on the current thread.
pub fn init() {
    crate::log_info!("[SUCCESS] Starting ENHANCED stealth overlay with Hotkeys.");
    
    // Run overlay on current thread (blocks until exit)
    // This is intentional - the overlay IS the main application loop
    unsafe {
        if let Err(e) = create_guaranteed_stealth_overlay() {
            crate::log_error!("[FATAL ERROR] Overlay creation failed: {:?}", e);
        }
    }
}

pub fn set_overlay_text(text: String) {
    set_status_message(text);
}

pub fn set_status_message(text: String) {
    if let Ok(mut status) = STATUS_MESSAGE.try_lock() {
        *status = text;
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        STATUS_EXPIRY.store(now + 3000, Ordering::SeqCst); // 3-second expiry
    }
    
    let hwnd_val = OVERLAY_HWND.load(Ordering::SeqCst);
    if hwnd_val != 0 {
        unsafe { InvalidateRect(HWND(hwnd_val), None, FALSE); }
    }
}

// 🎯 THROTTLE: Only repaint every 50ms to prevent jitter
static LAST_REPAINT_TIME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn append_ai_response(text: &str) {
    // Use try_lock to prevent blocking - if locked, we'll get it next time
    if let Ok(mut last_resp) = LAST_AI_RESPONSE.try_lock() {
        // Skip empty text
        if text.trim().is_empty() {
            return;
        }
        
        // SET the full text (caller sends accumulated response)
        *last_resp = text.to_string();
        
        // Also update OVERLAY_TEXT immediately
        if let Ok(mut overlay_text) = OVERLAY_TEXT.try_lock() {
            *overlay_text = text.to_string();
        }
    } else {
        return; // Skip if locked
    }
    
    // 🎯 THROTTLE REPAINT: Only repaint every 50ms to prevent jitter
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let last = LAST_REPAINT_TIME.load(Ordering::SeqCst);
    
    if now - last < 50 {
        return; // Skip repaint - too soon
    }
    LAST_REPAINT_TIME.store(now, Ordering::SeqCst);
    
    let hwnd_val = OVERLAY_HWND.load(Ordering::SeqCst);
    if hwnd_val != 0 {
        unsafe {
            InvalidateRect(HWND(hwnd_val), None, FALSE);
        }
    }
}

pub fn reset_ai_response() {
    // 🎯 Shift+Q: Clear ONLY the question display (LIVE_TRANSCRIPTION) 
    // so new transcription can start fresh while AI generates answer
    // OVERLAY_TEXT keeps showing AI response, LAST_AI_RESPONSE keeps history
    
    // Clear only the live transcription (question display)
    for _ in 0..10 {
        if let Ok(mut trans) = LIVE_TRANSCRIPTION.try_lock() {
            trans.clear();
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    
    SCROLL_OFFSET.store(0, Ordering::SeqCst); // Reset scroll
    
    // Force repaint
    let hwnd_val = OVERLAY_HWND.load(Ordering::SeqCst);
    if hwnd_val != 0 {
        unsafe { InvalidateRect(HWND(hwnd_val), None, FALSE); }
    }
}

/// 🧹 Clear EVERYTHING - used by Shift+A
pub fn clear_all_buffers() {
    // Clear ALL buffers
    for _ in 0..10 {
        if let Ok(mut last_resp) = LAST_AI_RESPONSE.try_lock() {
            last_resp.clear();
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    
    for _ in 0..10 {
        if let Ok(mut trans) = LIVE_TRANSCRIPTION.try_lock() {
            trans.clear();
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    
    for _ in 0..10 {
        if let Ok(mut overlay) = OVERLAY_TEXT.try_lock() {
            overlay.clear();
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    
    SCROLL_OFFSET.store(0, Ordering::SeqCst);
    
    // Force repaint
    let hwnd_val = OVERLAY_HWND.load(Ordering::SeqCst);
    if hwnd_val != 0 {
        unsafe { InvalidateRect(HWND(hwnd_val), None, TRUE); }
    }
}

/// 🎯 Set live transcription SNAPSHOT (Event Driven)
/// Replaces the entire buffer with a pre-formatted snapshot from the logic layer
pub fn set_live_transcription_snapshot(snapshot: String) {
    if let Ok(mut trans) = LIVE_TRANSCRIPTION.try_lock() {
        *trans = snapshot;
    }
    // Trigger repaint - FALSE prevents background flash (we handle clear in double buffer)
    let hwnd_val = OVERLAY_HWND.load(Ordering::SeqCst);
    if hwnd_val != 0 {
        unsafe {
            InvalidateRect(HWND(hwnd_val), None, FALSE);
        }
    }
}

/// Legacy wrapper
pub fn set_live_transcription(text: &str) {
    set_live_transcription_snapshot(text.to_string());
}

/// Get live transcription for display
pub fn get_live_transcription() -> String {
    if let Ok(trans) = LIVE_TRANSCRIPTION.try_lock() {
        trans.clone()
    } else {
        String::new()
    }
}

pub fn toggle_visibility() {
    let hwnd_val = OVERLAY_HWND.load(Ordering::SeqCst);
    if hwnd_val == 0 { return; }
    let hwnd = HWND(hwnd_val);

    unsafe {
        if OVERLAY_VISIBLE.load(Ordering::SeqCst) {
            ShowWindow(hwnd, SW_HIDE);
            OVERLAY_VISIBLE.store(false, Ordering::SeqCst);
        } else {
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            OVERLAY_VISIBLE.store(true, Ordering::SeqCst);
        }
    }
}

unsafe fn create_guaranteed_stealth_overlay() -> Result<()> {
    let instance = GetModuleHandleA(None)?;
    
    use rand::Rng;
    let mut rng = rand::thread_rng();
    
    let class_names = ["MSCTFIME", "IME", "OleMainThreadWndClass", "WorkerW", "Shell_TrayWnd"];
    let base_name = class_names[rng.gen_range(0..class_names.len())];
    let random_suffix: String = (0..4).map(|_| rng.sample(rand::distributions::Alphanumeric) as char).collect();
    let class_name_str = format!("{}_{}", base_name, random_suffix);
    let class_name = std::ffi::CString::new(class_name_str).unwrap();
    let class_name_pcstr = PCSTR(class_name.as_ptr() as *const u8);

    let wc = WNDCLASSA {
        hInstance: instance.into(),
        lpszClassName: class_name_pcstr,
        lpfnWndProc: Some(stealth_wnd_proc),
        hCursor: LoadCursorW(None, IDC_ARROW)?,
        ..Default::default()
    };

    RegisterClassA(&wc);
    
    // Use innocuous window title that looks like a system process
    // "Anti-Proctor Overlay" is too obvious in Task Manager!
    let window_title = s!("Windows Font Cache Service");

    // WS_EX_TRANSPARENT = click passes through to windows below
    let hwnd = CreateWindowExA(
        WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT,
        class_name_pcstr,
        window_title,
        WS_POPUP | WS_VISIBLE,
        100, 100, 800, 600,
        None, None, instance, None,
    );

    if hwnd.0 == 0 { return Err(Error::from_win32()); }
    OVERLAY_HWND.store(hwnd.0, Ordering::SeqCst);
    
    // 🔲 ROUNDED CORNERS - Premium look (20px radius)
    let rgn = CreateRoundRectRgn(0, 0, 800, 600, 24, 24);
    SetWindowRgn(hwnd, rgn, TRUE);
    
    // 🌟 WATER-LIKE TRANSPARENCY - 215 alpha (~84% opacity, brighter text overlap)
    SetLayeredWindowAttributes(hwnd, COLORREF(0), 215, LWA_ALPHA)?;
    ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    
    // Force window to foreground initially
    SetForegroundWindow(hwnd);
    crate::log_info!("[DEBUG] Window created: High-Contrast Glass Mode, Alpha 215");

    // Stealth mode ENABLED - Window hidden from screen capture/recording
    let mut result = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
    if result.is_ok() {
        crate::log_info!("[SUCCESS] Applied WDA_EXCLUDEFROMCAPTURE - Stealth ON");
    } else {
        result = SetWindowDisplayAffinity(hwnd, WDA_MONITOR);
        if result.is_ok() {
             crate::log_info!("[SUCCESS] Applied WDA_MONITOR - Stealth ON (fallback)");
        } else {
             crate::log_error!("[ERROR] Failed anti-capture: {:?}", Error::from_win32());
        }
    }

    // =================================================================
    // 🎹 HOTKEY REGISTRATION (using named constants)
    // =================================================================
    
    // Priority Actions (Shift + Key)
    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_Q as i32, MOD_SHIFT, 0x51);  // Q - Send to AI
    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_A as i32, MOD_SHIFT, 0x41);  // A - Clear ALL
    
    // Screenshot Workflow
    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_S as i32, MOD_SHIFT, 0x53);  // S - Capture
    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_D as i32, MOD_SHIFT, 0x44);  // D - Delete all
    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_F as i32, MOD_SHIFT, 0x46);  // F - Send/Flash
    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_M as i32, MOD_SHIFT, 0x4D);  // M - Cycle model
    
    // UI Control
    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_U as i32, MOD_SHIFT, 0x55);  // U - Scroll UP
    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_N as i32, MOD_SHIFT, 0x4E);  // N - Scroll DOWN
    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_T as i32, MOD_SHIFT, 0x54);  // T - Toggle visibility
    let _ = RegisterHotKey(hwnd, HOTKEY_SHIFT_P as i32, MOD_SHIFT, 0x50);  // P - Toggle (legacy)
    
    // Alt+Shift Combinations
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_TEST as i32, MOD_ALT | MOD_SHIFT, 0x54);  // T - Test
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_S as i32, MOD_ALT | MOD_SHIFT, 0x53);     // S
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_A as i32, MOD_ALT | MOD_SHIFT, 0x41);     // A
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_D as i32, MOD_ALT | MOD_SHIFT, 0x44);     // D
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_LEFT as i32, MOD_ALT | MOD_SHIFT, VK_LEFT.0 as u32);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_RIGHT as i32, MOD_ALT | MOD_SHIFT, VK_RIGHT.0 as u32);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_UP as i32, MOD_ALT | MOD_SHIFT, VK_UP.0 as u32);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_DOWN as i32, MOD_ALT | MOD_SHIFT, VK_DOWN.0 as u32);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_W as i32, MOD_ALT | MOD_SHIFT, 0x57);     // W - Height+
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_X as i32, MOD_ALT | MOD_SHIFT, 0x58);     // X - Height-
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_PLUS as i32, MOD_ALT | MOD_SHIFT, VK_ADD.0 as u32);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_MINUS as i32, MOD_ALT | MOD_SHIFT, VK_SUBTRACT.0 as u32);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_OEM_PLUS as i32, MOD_ALT | MOD_SHIFT, VK_OEM_PLUS.0 as u32);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_OEM_MINUS as i32, MOD_ALT | MOD_SHIFT, VK_OEM_MINUS.0 as u32);
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_K as i32, MOD_ALT | MOD_SHIFT, 0x4B);     // K - Kill
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_I as i32, MOD_ALT | MOD_SHIFT, 0x49);     // I - AutoType
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_B as i32, MOD_ALT | MOD_SHIFT, 0x42);     // B - Auto-Bracket
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_M as i32, MOD_ALT | MOD_SHIFT, 0x4D);     // M - Model
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_V as i32, MOD_ALT | MOD_SHIFT, 0x56);     // V - Voice
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_C as i32, MOD_ALT | MOD_SHIFT, 0x43);     // C - Close Live
    let _ = RegisterHotKey(hwnd, HOTKEY_ALT_SHIFT_Z as i32, MOD_ALT | MOD_SHIFT, 0x5A);     // Z - Manual Clear
    
    crate::log_info!("[HOTKEY] All hotkeys registered successfully");
    SetTimer(hwnd, TIMER_HEARTBEAT, 5000, None);

    let mut message = MSG::default();
    crate::log_info!("Entering Message Loop");
    while GetMessageA(&mut message, None, 0, 0).into() {
        TranslateMessage(&message);
        DispatchMessageA(&message);
    }
    crate::log_info!("Exiting Message Loop");
    Ok(())
}

unsafe extern "system" fn stealth_wnd_proc(
    window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM
) -> LRESULT {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        match message {
            WM_TIMER => {
                if wparam.0 == TIMER_HEARTBEAT {
                    // Heartbeat - uncomment for debugging
                    // crate::log_info!("HEARTBEAT: Overlay is alive.");
                }
                LRESULT(0)
            }
            WM_HOTKEY => {
                let id = wparam.0;
                crate::log_info!("WM_HOTKEY received: ID {}", id);
                match id {
                    HOTKEY_ALT_SHIFT_TOGGLE => { toggle_visibility(); },
                    HOTKEY_ALT_SHIFT_TEST => { set_overlay_text("Test!".to_string()); },
                    HOTKEY_ALT_SHIFT_S => { 
                        std::thread::spawn(|| {
                            crate::capture::take_screenshot();
                            let count = crate::capture::get_screenshot_count();
                            set_status_message(format!("📸 Screenshot #{} taken!", count));
                        });
                    },
                    HOTKEY_ALT_SHIFT_A => {
                        let count = crate::capture::get_screenshot_count();
                        if count > 0 {
                            set_overlay_text(format!("🔄 Analyzing {} screenshots with AI...", count));
                            std::thread::spawn(|| {
                                let screenshots = crate::capture::get_all_screenshots();
                                crate::log_info!("Querying AI with {} screenshots", screenshots.len());
                                let response = crate::ai::ask_ai_with_images("Analyze these images", screenshots, |streaming_text| {
                                    crate::overlay::set_overlay_text(streaming_text);
                                });
                                
                                // Save response for auto-type safely
                                if let Ok(mut last_resp) = LAST_AI_RESPONSE.lock() {
                                    *last_resp = response.clone();
                                } else if let Ok(mut last_resp) = LAST_AI_RESPONSE.try_lock() {
                                    *last_resp = response.clone();
                                }
                                crate::overlay::set_overlay_text(response);
                            });
                        } else {
                            set_overlay_text("❌ No screenshots!\nPress Alt+Shift+S first".to_string());
                        }
                    },
                    HOTKEY_ALT_SHIFT_D => {
                        std::thread::spawn(|| {
                            crate::capture::delete_all_screenshots();
                            set_status_message("🗑️ All screenshots deleted!".to_string());
                        });
                    },
                    HOTKEY_ALT_SHIFT_LEFT => { move_overlay(-50, 0); },
                    HOTKEY_ALT_SHIFT_RIGHT => { move_overlay(50, 0); },
                    HOTKEY_ALT_SHIFT_UP => { move_overlay(0, -50); },
                    HOTKEY_ALT_SHIFT_DOWN => { move_overlay(0, 50); },
                    HOTKEY_ALT_SHIFT_W => { scroll_overlay(-30); },
                    HOTKEY_ALT_SHIFT_X => { scroll_overlay(30); },
                    HOTKEY_ALT_SHIFT_PLUS | HOTKEY_ALT_SHIFT_OEM_PLUS => { resize_overlay(50, 50); },
                    HOTKEY_ALT_SHIFT_MINUS | HOTKEY_ALT_SHIFT_OEM_MINUS => { resize_overlay(-50, -50); },
                    HOTKEY_ALT_SHIFT_K => {
                        crate::log_info!("Kill hotkey - Exiting application");
                        set_overlay_text("Exiting...".to_string());
                        
                        // Request graceful shutdown
                        crate::request_shutdown();
                        
                        // Post quit message to exit message loop
                        PostQuitMessage(0);
                    },

                    HOTKEY_ALT_SHIFT_I => {
                        if crate::autotype::is_typing() {
                            crate::autotype::stop_typing();
                            set_overlay_text("Auto-type stopped.".to_string());
                        } else {
                            // Safe lock handling
                            let text_to_type = match LAST_AI_RESPONSE.lock() {
                                Ok(guard) => guard.clone(),
                                Err(poisoned) => poisoned.into_inner().clone(),
                            };
                            
                            if !text_to_type.is_empty() {
                                set_overlay_text("Auto-typing started...".to_string());
                                crate::autotype::start_typing(text_to_type);
                            } else {
                                set_overlay_text("No AI response to type!".to_string());
                            }
                        }
                    },

                    HOTKEY_ALT_SHIFT_B => {
                        // Toggle auto-bracket mode
                        let new_state = crate::autotype::toggle_auto_bracket_mode();
                        if new_state {
                            set_overlay_text("🔧 Auto-Bracket: ON\n(Delete key will remove auto-inserted brackets)".to_string());
                        } else {
                            set_overlay_text("🔧 Auto-Bracket: OFF\n(No auto-bracket compensation)".to_string());
                        }
                    },

                    HOTKEY_ALT_SHIFT_M => {
                        // Cycle AI model in background
                        std::thread::spawn(|| {
                            let new_model = crate::ai::cycle_model();
                            set_status_message(format!("🤖 AI Model: {}", new_model.name()));
                        });
                    },
                    HOTKEY_ALT_SHIFT_V => { // V - Live Mode Start (Google WSS)
                        if !crate::audio::is_live_streaming() {
                             crate::overlay::set_overlay_text("🚀 Connecting to Live API...".to_string());
                             crate::overlay::stealth::reset_ai_response(); // Clear previous response
                             
                             // Use std::thread + block_on (same fix as AI images)
                             std::thread::spawn(|| {
                                 crate::log_info!("Live session thread started...");
                                 let result = crate::TOKIO_RT.block_on(async {
                                     crate::ai::live_client::start_live_session(|text| {
                                         crate::overlay::stealth::append_ai_response(&text);
                                     }).await
                                 });
                                 
                                 match result {
                                     Ok(_) => {
                                         crate::log_info!("Live session ended gracefully");
                                     },
                                     Err(e) => {
                                         crate::log_error!("Live session error: {}", e);
                                         crate::overlay::set_overlay_text(format!("❌ Error: {}", e));
                                     }
                                 }
                             });
                        } else {
                            crate::overlay::set_overlay_text("⚠️ Already connected!".to_string());
                        }
                    },
                    HOTKEY_ALT_SHIFT_C => { // C - Close Live
                        if crate::audio::is_live_streaming() {
                            crate::audio::IS_LIVE_STREAMING.store(false, Ordering::SeqCst);
                            crate::overlay::set_overlay_text("🛑 Closing connection...".to_string());
                        } else {
                            crate::overlay::set_overlay_text("❌ Not connected.".to_string());
                        }
                    },
                    HOTKEY_ALT_SHIFT_Z => { // Z - Manual Clear
                         crate::overlay::stealth::reset_ai_response();
                         crate::overlay::set_overlay_text("".to_string());
                    },
                    HOTKEY_SHIFT_Q => { // 🎯 Shift+Q - PRIORITY: Send transcription to AI NOW!
                        crate::log_info!("⚡ Shift+Q pressed - triggering AI NOW");
                        crate::overlay::set_overlay_text("⚡ Requesting Answer...".to_string());
                        crate::ai::live_client::trigger_answer_now();
                    },
                    HOTKEY_SHIFT_A => { // 🧹 Shift+A - Clear Everything
                        crate::log_info!("🧹 Shift+A pressed - CLEARING ALL");
                        set_status_message("🧹 Cleared.".to_string());
                        crate::ai::live_client::force_clear_buffers();
                    },
                    HOTKEY_SHIFT_S => { // 📸 Shift+S - Screenshot
                        std::thread::spawn(|| {
                            crate::capture::take_screenshot();
                            let count = crate::capture::get_screenshot_count();
                            set_status_message(format!("📸 Screenshot #{} captured!", count));
                        });
                    },
                    HOTKEY_SHIFT_D => { // 🗑️ Shift+D - Clear Snapshots
                        std::thread::spawn(|| {
                            crate::capture::delete_all_screenshots();
                            set_status_message("🗑️ All snapshots deleted!".to_string());
                        });
                    },
                    HOTKEY_SHIFT_F => { // 🚀 Shift+F - Send to AI
                        set_status_message("⏳ Analyzing screen...".to_string());
                        reset_ai_response(); 
                        crate::ai::live_client::trigger_screenshot_analysis(|text| {
                            append_ai_response(&text);
                        });
                    },
                    HOTKEY_SHIFT_M => { // 🤖 Shift+M - Cycle Model
                        std::thread::spawn(|| {
                            let new_model = crate::ai::cycle_model();
                            set_status_message(format!("🤖 Model: {}", new_model.name()));
                        });
                    },
                    HOTKEY_SHIFT_U => { // ⬆️ Shift+U - Scroll Up
                        scroll_overlay(-100);
                    },
                    HOTKEY_SHIFT_N => { // ⬇️ Shift+N - Scroll Down
                        scroll_overlay(100);
                    },
                    HOTKEY_SHIFT_T | HOTKEY_SHIFT_P => { // 👁️ Shift+T / Shift+P - Toggle Visibility
                        toggle_visibility();
                    },
                    _ => {}
                }
                LRESULT(0)
            }
            WM_CREATE => {
                let cache = unsafe { GdiCache::new() };
                if let Ok(mut gdi) = GDI_CACHE.lock() {
                    *gdi = Some(cache);
                }
                LRESULT(0)
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let screen_dc = BeginPaint(window, &mut ps);
                if !screen_dc.is_invalid() {
                    let mut client_rect = RECT::default();
                    GetClientRect(window, &mut client_rect);
                    let w = client_rect.right;
                    let h = client_rect.bottom;
                    if w <= 0 || h <= 0 { EndPaint(window, &ps); return LRESULT(0); }

                    // Double Buffer
                    let mem_dc = CreateCompatibleDC(screen_dc);
                    let mem_bitmap = CreateCompatibleBitmap(screen_dc, w, h);
                    let old_bitmap = SelectObject(mem_dc, mem_bitmap);

                    // Fetch Cached GDI Objects
                    let gdi_lock = GDI_CACHE.lock().unwrap();
                    let gdi = gdi_lock.as_ref().unwrap();

                    // ==== ⚪ HIGH-CLARITY WATER GLASS THEME ====
                    let color_header_q   = 0x00EEEEEE;  // Near-white for question headers
                    let color_header_ai  = 0x00FFFFFF;  // Pure white for AI headers
                    let color_text_white = 0x00FFFFFF;  // Pure white - maximum readability
                    let color_text_cream = 0x00FFFFFF;  // Pure white - maximum readability
                    let color_bullet     = 0x00DDDDDD;  // Bright silver bullets
                    let color_code       = 0x00F0F0F0;  // High-brightness code text
                    let color_text_shadow = 0x00000000; // Pure black shadow

                    // 1. Base Background - Semi-transparent dark
                    FillRect(mem_dc, &client_rect, gdi.br_bg);
                    
                    // 2. Simple thin neutral border
                    let border_rect = RECT { left: 0, right: w, top: 0, bottom: h };
                    FrameRect(mem_dc, &border_rect, gdi.br_border);

                    SetBkMode(mem_dc, TRANSPARENT);
                    
                    let ai_text = OVERLAY_TEXT.try_lock().map(|g| g.clone()).unwrap_or_default();
                    let live_trans = LIVE_TRANSCRIPTION.try_lock().ok().and_then(|g| if g.is_empty() { None } else { Some(g.clone()) });

                    let pad = 20;
                    let scroll_off = SCROLL_OFFSET.load(Ordering::SeqCst);
                    let mut y = 45 - scroll_off as i32; // Lowered start for status area

                    // == 🏷️ STATUS AREA (Fixed Top) ==
                    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
                    let expiry = STATUS_EXPIRY.load(Ordering::SeqCst);
                    
                    if now < expiry {
                        if let Ok(status) = STATUS_MESSAGE.try_lock() {
                            if !status.is_empty() {
                                SelectObject(mem_dc, gdi.font_bold);
                                let stxt: Vec<u16> = OsStr::new(&*status).encode_wide().chain(Some(0)).collect();
                                
                                // Shadow
                                SetTextColor(mem_dc, COLORREF(color_text_shadow));
                                let mut sr_shadow = RECT { left: pad + 1, right: w - pad + 1, top: 11, bottom: 41 };
                                DrawTextW(mem_dc, &mut stxt.clone(), &mut sr_shadow, DT_CENTER | DT_SINGLELINE);

                                // Main
                                SetTextColor(mem_dc, COLORREF(0x00AAEEFF)); // Cyan
                                let mut sr = RECT { left: pad, right: w - pad, top: 10, bottom: 40 };
                                DrawTextW(mem_dc, &mut stxt.clone(), &mut sr, DT_CENTER | DT_SINGLELINE);
                            }
                        }
                    }

                    // == 🎤 INTERVIEWER CARD (Question) ==
                    if let Some(q) = live_trans {
                        // Card background first
                        SelectObject(mem_dc, gdi.font_main);
                        let mut measure_rect = RECT { left: pad + 28, right: w - pad - 16, top: 0, bottom: h };
                        let txt: Vec<u16> = OsStr::new(&q).encode_wide().chain(Some(0)).collect();
                        let th = DrawTextW(mem_dc, &mut txt.clone(), &mut measure_rect, DT_CALCRECT | DT_WORDBREAK);
                        
                        let card_rect = RECT { left: pad, right: w - pad, top: y, bottom: y + th + 60 };
                        
                        // 🌟 PREMIUM CARD with shadow effect (dark border first)
                        let shadow_rect = RECT { left: pad + 3, right: w - pad + 3, top: y + 3, bottom: card_rect.bottom + 3 };
                        FillRect(mem_dc, &shadow_rect, gdi.br_bg);
                        FillRect(mem_dc, &card_rect, gdi.br_card);
                        
                        // 🔶 LEFT ACCENT BAR (thick, vibrant gold)
                        let accent_rect = RECT { left: pad, right: pad + 6, top: y, bottom: card_rect.bottom };
                        FillRect(mem_dc, &accent_rect, gdi.left_accent_brushes[0]);
                        
                        // Card border (subtle glow)
                        FrameRect(mem_dc, &card_rect, gdi.br_border);
                        
                        // Header with icon - BOLD + SHADOW
                        SelectObject(mem_dc, gdi.font_bold);
                        let hdr: Vec<u16> = OsStr::new("🎤 LISTENING...").encode_wide().chain(Some(0)).collect();
                        
                        // Shadow
                        SetTextColor(mem_dc, COLORREF(color_text_shadow));
                        let mut hr_shadow = RECT { left: pad + 19, right: w - pad + 1, top: y + 13, bottom: y + 37 };
                        DrawTextW(mem_dc, &mut hdr.clone(), &mut hr_shadow, DT_LEFT | DT_SINGLELINE);
                        
                        // Main
                        SetTextColor(mem_dc, COLORREF(color_header_q));
                        let mut hr = RECT { left: pad + 18, right: w - pad, top: y + 12, bottom: y + 36 };
                        DrawTextW(mem_dc, &mut hdr.clone(), &mut hr, DT_LEFT | DT_SINGLELINE);
                        
                        // Question text - bright white + SHADOW
                        SelectObject(mem_dc, gdi.font_main);
                        
                        // Shadow
                        SetTextColor(mem_dc, COLORREF(color_text_shadow));
                        let mut tr_shadow = RECT { left: pad + 29, right: w - pad - 15, top: y + 43, bottom: card_rect.bottom - 11 };
                        DrawTextW(mem_dc, &mut txt.clone(), &mut tr_shadow, DT_WORDBREAK);

                        // Main
                        SetTextColor(mem_dc, COLORREF(color_text_white));
                        let mut tr = RECT { left: pad + 28, right: w - pad - 16, top: y + 42, bottom: card_rect.bottom - 12 };
                        DrawTextW(mem_dc, &mut txt.clone(), &mut tr, DT_WORDBREAK);
                        
                        y = card_rect.bottom + 20;
                    }

                    // == 🤖 AI RESPONSE CARD ==
                    if !ai_text.is_empty() {
                        // Measure text height
                        SelectObject(mem_dc, gdi.font_main);
                        let mut measure_rect = RECT { left: pad + 28, right: w - pad - 16, top: 0, bottom: 1000000 };
                        let txt: Vec<u16> = OsStr::new(&ai_text).encode_wide().chain(Some(0)).collect();
                        let th = DrawTextW(mem_dc, &mut txt.clone(), &mut measure_rect, DT_CALCRECT | DT_WORDBREAK);
                        
                        let card_rect = RECT { left: pad, right: w - pad, top: y, bottom: y + th + 60 };
                        
                        // 🌟 PREMIUM CARD with shadow
                        let shadow_rect = RECT { left: pad + 3, right: w - pad + 3, top: y + 3, bottom: card_rect.bottom + 3 };
                        FillRect(mem_dc, &shadow_rect, gdi.br_bg);
                        FillRect(mem_dc, &card_rect, gdi.br_card_ai);
                        
                        // 🟢 LEFT ACCENT BAR (thick, vibrant cyan)
                        let accent_rect = RECT { left: pad, right: pad + 5, top: y, bottom: card_rect.bottom };
                        FillRect(mem_dc, &accent_rect, gdi.left_accent_brushes[1]);
                        
                        // Card border (subtle glow)
                        FrameRect(mem_dc, &card_rect, gdi.br_border);
                        
                        // Header with icon - BOLD + SHADOW
                        SelectObject(mem_dc, gdi.font_bold);
                        let model_name = crate::ai::get_current_model_name();
                        let header_text = format!("🤖 AI SUGGESTION ({})", model_name);
                        let hdr: Vec<u16> = OsStr::new(&header_text).encode_wide().chain(Some(0)).collect();

                        // Shadow
                        SetTextColor(mem_dc, COLORREF(color_text_shadow));
                        let mut hr_shadow = RECT { left: pad + 19, right: w - pad + 1, top: y + 13, bottom: y + 37 };
                        DrawTextW(mem_dc, &mut hdr.clone(), &mut hr_shadow, DT_LEFT | DT_SINGLELINE);
                        
                        // Main
                        SetTextColor(mem_dc, COLORREF(color_header_ai));
                        let mut hr = RECT { left: pad + 18, right: w - pad, top: y + 12, bottom: y + 36 };
                        DrawTextW(mem_dc, &mut hdr.clone(), &mut hr, DT_LEFT | DT_SINGLELINE);
                        
                        // AI Text - Bright White + SHADOW
                        SelectObject(mem_dc, gdi.font_main);
                        
                        // Shadow
                        SetTextColor(mem_dc, COLORREF(color_text_shadow));
                        let mut tr_shadow = RECT { left: pad + 29, right: w - pad - 15, top: y + 43, bottom: card_rect.bottom - 11 };
                        DrawTextW(mem_dc, &mut txt.clone(), &mut tr_shadow, DT_WORDBREAK);

                        // Main
                        SetTextColor(mem_dc, COLORREF(color_text_white)); // Pure white
                        let mut tr = RECT { left: pad + 28, right: w - pad - 16, top: y + 42, bottom: card_rect.bottom - 12 };
                        DrawTextW(mem_dc, &mut txt.clone(), &mut tr, DT_WORDBREAK);
                    }

                    BitBlt(screen_dc, 0, 0, w, h, mem_dc, 0, 0, SRCCOPY);

                    SelectObject(mem_dc, old_bitmap);
                    DeleteObject(mem_bitmap);
                    DeleteDC(mem_dc);
                }
                EndPaint(window, &ps);
                LRESULT(0)
            }
            WM_DESTROY => {
                crate::log_info!("WM_DESTROY received - cleaning up resources");
                // Take and drop the GDI cache - Drop trait will cleanup automatically
                if let Ok(mut gdi_opt) = GDI_CACHE.lock() {
                    let _ = gdi_opt.take(); // Drop triggers cleanup
                }
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcA(window, message, wparam, lparam),
        }
    }));

    match result {
        Ok(lresult) => lresult,
        Err(e) => {
             crate::log_error!("PANIC caught in stealth_wnd_proc: {:?}", e);
             // Return 0 or default to keep alive if possible, or DefWindowProc
             DefWindowProcA(window, message, wparam, lparam)
        }
    }
}

pub fn scroll_overlay(dy: i32) {
    let current = SCROLL_OFFSET.load(Ordering::SeqCst);
    let mut new_offset = current + dy as isize;
    
    // Clamp to minimum 0
    if new_offset < 0 { new_offset = 0; }
    
    SCROLL_OFFSET.store(new_offset, Ordering::SeqCst);
    
    let hwnd_val = OVERLAY_HWND.load(Ordering::SeqCst);
    if hwnd_val != 0 {
        unsafe { InvalidateRect(HWND(hwnd_val), None, TRUE); }
    }
}

pub fn move_overlay(dx: i32, dy: i32) {
    let hwnd_val = OVERLAY_HWND.load(Ordering::SeqCst);
    if hwnd_val == 0 { return; }
    let hwnd = HWND(hwnd_val);
    
    unsafe {
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_ok() {
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            
            SetWindowPos(
                hwnd, HWND(0), 
                rect.left + dx, rect.top + dy, 
                width, height, 
                SWP_NOZORDER | SWP_NOACTIVATE
            );
        }
    }
}

pub fn resize_overlay(dw: i32, dh: i32) {
    let hwnd_val = OVERLAY_HWND.load(Ordering::SeqCst);
    if hwnd_val == 0 { return; }
    let hwnd = HWND(hwnd_val);
    
    unsafe {
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_ok() {
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            let new_w = width + dw;
            let new_h = height + dh;
            
            SetWindowPos(
                hwnd, HWND(0), 
                rect.left, rect.top, 
                new_w, new_h, 
                SWP_NOZORDER | SWP_NOACTIVATE
            );
            
            // Update rounded corners for new size
            let rgn = CreateRoundRectRgn(0, 0, new_w, new_h, 24, 24);
            SetWindowRgn(hwnd, rgn, TRUE);
        }
    }
}

pub fn clear_overlay_text() {}
