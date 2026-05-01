🧠 Architecture (your case)
React (UI)
   ↓ invoke()
Tauri (Rust commands)
   ↓ HTTP call
Ollama (localhost:11434)
   ↓
Model (phi)
⚙️ Step 1: Make sure Ollama is running
ollama serve
🦀 Step 2: Tauri backend (Rust)

Go to:

src-tauri/src/main.rs
Add command
use tauri::command;

#[command]
async fn ask_ai(prompt: String) -> Result<String, String> {
    let client = reqwest::Client::new();

    let res = client
        .post("http://localhost:11434/api/generate")
        .json(&serde_json::json!({
            "model": "phi",
            "prompt": format!(
                "You are a coding assistant. Give only correct code.\n{}",
                prompt
            ),
            "stream": false
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let json: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;

    Ok(json["response"].as_str().unwrap_or("").to_string())
}
Register command
fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![ask_ai])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
📦 Step 3: Add dependencies

In Cargo.toml:

[dependencies]
tauri = { version = "1", features = ["api-all"] }
reqwest = { version = "0.11", features = ["json"] }
serde_json = "1"
⚛️ Step 4: React frontend
Call Tauri command
import { invoke } from "@tauri-apps/api/tauri";

async function askAI() {
  const response = await invoke("ask_ai", {
    prompt: "Leetcode 3Sum in JavaScript"
  });

  console.log(response);
}
⚡ Step 5: UI example
import { useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";

export default function App() {
  const [input, setInput] = useState("");
  const [output, setOutput] = useState("");

  const handleAsk = async () => {
    const res = await invoke("ask_ai", { prompt: input });
    setOutput(res);
  };

  return (
    <div>
      <textarea onChange={(e) => setInput(e.target.value)} />
      <button onClick={handleAsk}>Ask</button>
      <pre>{output}</pre>
    </div>
  );
}
⚠️ Important (don’t skip)
1. Allow localhost in Tauri

In tauri.conf.json:

"security": {
  "csp": null
}
2. Ollama must be running BEFORE app

Otherwise:

connection refused