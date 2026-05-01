# 🎯 COMPLETE TODO: Build Undetectable Anti-Proctoring App
## Security Research Project - Step-by-Step Implementation Guide

---

## ⚠️ AUTHORIZATION REQUIRED
This TODO is for authorized security research (hackathon/penetration testing) ONLY.
Ensure you have written permission before proceeding.

---

# 📋 PHASE 1: SETUP & FOUNDATION (Days 1-2)

## Day 1: Development Environment

### TODO 1.1: Install Rust Toolchain
- [ ] Visit https://rustup.rs
- [ ] Download and run rustup-init.exe
- [ ] Select default installation (stable toolchain)
- [ ] Verify installation: Open CMD, run `rustc --version`
- [ ] Install Cargo: Should come with Rust
- [ ] Verify Cargo: Run `cargo --version`

### TODO 1.2: Install Windows Development Tools
- [ ] Download Visual Studio Build Tools 2022
- [ ] During install, select: "Desktop development with C++"
- [ ] Install Windows 10/11 SDK (latest version)
- [ ] Install WDK (Windows Driver Kit) - Optional but recommended
- [ ] Verify: Run `cl.exe` in Developer Command Prompt

### TODO 1.3: Setup Code Editor
- [ ] Install VS Code from https://code.visualstudio.com
- [ ] Install extensions:
  - [ ] rust-analyzer (official Rust language server)
  - [ ] CodeLLDB (debugger)
  - [ ] Better TOML (for Cargo.toml)
  - [ ] Error Lens (show errors inline)

### TODO 1.4: Create Project Structure
```bash
# Run these commands:
cargo new anti-proctor --bin
cd anti-proctor
mkdir src/stealth src/capture src/ai src/overlay src/network
```

- [ ] Create folder structure as shown above
- [ ] Initialize git repo: `git init` (keep private!)
- [ ] Create `.gitignore` file with Rust template

### TODO 1.5: Setup Dependencies in Cargo.toml
Add these to your `Cargo.toml`:
```toml
[dependencies]
windows = { version = "0.52", features = ["Win32_Foundation", "Win32_System_Threading", "Win32_Graphics_Dxgi", "Win32_Graphics_Direct3D11"] }
tokio = { version = "1.35", features = ["full"] }
imgui = "0.11"
imgui-winit-support = "0.11"
winit = "0.29"
tesseract = "0.13"
image = "0.24"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
reqwest = { version = "0.11", features = ["json", "rustls-tls"] }
```

- [ ] Copy dependencies to Cargo.toml
- [ ] Run `cargo build` to download and compile
- [ ] Fix any compilation errors
- [ ] Verify build completes successfully

---

# 📋 PHASE 2: STEALTH LAYER (Days 3-5)

## Day 3: Process Hiding

### TODO 2.1: Implement Process Name Randomization
File: `src/stealth/process_hiding.rs`

- [ ] Generate random legitimate process name at runtime
- [ ] Options: "svchost.exe", "dwm.exe", "RuntimeBroker.exe"
- [ ] Store original path before rename
- [ ] Implement self-rename function
- [ ] Add randomization on each startup

**Implementation checklist:**
```rust
// Create these functions:
- [ ] generate_random_system_name() -> String
- [ ] get_system_process_list() -> Vec<String>
- [ ] rename_self_process(new_name: &str) -> Result<()>
- [ ] set_process_description() // Set "Microsoft Windows Service"
```

### TODO 2.2: Process Injection Implementation
File: `src/stealth/injection.rs`

- [ ] Find target process (explorer.exe recommended)
- [ ] Get process handle with PROCESS_ALL_ACCESS
- [ ] Allocate memory in target process (VirtualAllocEx)
- [ ] Write your code to allocated memory (WriteProcessMemory)
- [ ] Create remote thread in target (CreateRemoteThread)
- [ ] Verify injection succeeded
- [ ] Clean up handles

**Function checklist:**
```rust
- [ ] find_process_by_name(name: &str) -> Option<u32>
- [ ] inject_into_process(pid: u32, dll_path: &str) -> Result<()>
- [ ] create_hollow_process(target: &str) -> Result<()>
- [ ] verify_injection_success() -> bool
```

### TODO 2.3: Hide from Task Manager
File: `src/stealth/task_manager.rs`

- [ ] Implement parent process spoofing
- [ ] Set process to SYSTEM owner if possible
- [ ] Mark as critical system process (can't be terminated)
- [ ] Hide window from window lists
- [ ] Remove from taskbar enumeration

**Checklist:**
```rust
- [ ] spoof_parent_process(ppid: u32) -> Result<()>
- [ ] set_critical_process() -> Result<()>
- [ ] hide_from_window_list() -> Result<()>
- [ ] remove_taskbar_entry() -> Result<()>
```

### TODO 2.4: Anti-Debugging Protection
File: `src/stealth/anti_debug.rs`

- [ ] Check IsDebuggerPresent() on startup
- [ ] Check for remote debuggers
- [ ] Detect common debugging tools (x64dbg, IDA, etc.)
- [ ] Exit immediately if debugger detected
- [ ] Add timing checks for VM detection

**Checklist:**
```rust
- [ ] is_debugger_present() -> bool
- [ ] check_remote_debugger() -> bool
- [ ] detect_debugging_tools() -> Vec<String>
- [ ] detect_vm_environment() -> bool
- [ ] exit_if_unsafe_environment()
```

## Day 4: Persistence & Autostart

### TODO 2.5: Windows Service Registration
File: `src/stealth/service.rs`

- [ ] Create service installation function
- [ ] Set service name: "WindowsSecurityUpdateService"
- [ ] Set display name: Legitimate-sounding Microsoft service
- [ ] Configure: Automatic start (delayed)
- [ ] Set description: Windows system component
- [ ] Run as SYSTEM account if possible
- [ ] Test: Verify service appears in services.msc

**Checklist:**
```rust
- [ ] create_windows_service(name: &str, path: &str) -> Result<()>
- [ ] configure_service_autostart() -> Result<()>
- [ ] set_service_description(desc: &str) -> Result<()>
- [ ] start_service() -> Result<()>
- [ ] stop_service() -> Result<()>
- [ ] uninstall_service() -> Result<()>
```

### TODO 2.6: Scheduled Task Setup
File: `src/stealth/scheduler.rs`

- [ ] Create scheduled task via COM API
- [ ] Task name: "\Microsoft\Windows\SystemMaintenance\UpdateCheck"
- [ ] Trigger: At user logon
- [ ] Run with highest privileges
- [ ] Hidden: Check "Hidden" option
- [ ] Multiple instances: Run parallel if needed

**Checklist:**
```rust
- [ ] create_scheduled_task(name: &str, exe_path: &str) -> Result<()>
- [ ] set_task_trigger_logon() -> Result<()>
- [ ] set_task_hidden(hidden: bool) -> Result<()>
- [ ] set_highest_privileges() -> Result<()>
```

### TODO 2.7: Registry Persistence
File: `src/stealth/registry.rs`

- [ ] Add to HKCU\...\Run key as backup
- [ ] Key name: "SecurityHealthSystray"
- [ ] Value: Path to executable
- [ ] Also add to HKLM\Run if admin available
- [ ] Create hidden registry values using special names

**Checklist:**
```rust
- [ ] add_registry_run_key(name: &str, path: &str) -> Result<()>
- [ ] remove_registry_run_key(name: &str) -> Result<()>
- [ ] verify_registry_persistence() -> bool
```

## Day 5: File System Stealth

### TODO 2.8: Install to Hidden Location
File: `src/stealth/installation.rs`

- [ ] Choose install location: `C:\ProgramData\Microsoft\Windows\SystemData\`
- [ ] Create hidden folder (FILE_ATTRIBUTE_HIDDEN)
- [ ] Copy executable with system name
- [ ] Set file attributes: System + Hidden
- [ ] Alternative: Use NTFS Alternate Data Stream (ADS)
- [ ] Store config in same hidden location

**Checklist:**
```rust
- [ ] get_install_directory() -> PathBuf
- [ ] create_hidden_directory(path: &Path) -> Result<()>
- [ ] copy_self_to_location(dest: &Path) -> Result<()>
- [ ] set_file_hidden_system(path: &Path) -> Result<()>
- [ ] install_to_ads(host_file: &Path) -> Result<()>
```

### TODO 2.9: Self-Protection Mechanisms
File: `src/stealth/protection.rs`

- [ ] Monitor for file deletion attempts
- [ ] Recreate if deleted
- [ ] Lock file while running (prevent deletion)
- [ ] Backup copy in different location
- [ ] Watchdog process (restarts if killed)

**Checklist:**
```rust
- [ ] lock_executable_file() -> Result<()>
- [ ] monitor_file_integrity() // Background thread
- [ ] create_backup_copy(dest: &Path) -> Result<()>
- [ ] spawn_watchdog_process() -> Result<()>
```

---

# 📋 PHASE 3: SCREEN CAPTURE (Days 6-8)

## Day 6: DXGI Desktop Duplication

### TODO 3.1: Initialize DirectX Components
File: `src/capture/dxgi_capture.rs`

- [ ] Create DXGI Factory (CreateDXGIFactory1)
- [ ] Enumerate adapters (graphics cards)
- [ ] Select primary adapter (adapter 0)
- [ ] Enumerate outputs (monitors)
- [ ] Select primary output (output 0)
- [ ] Create D3D11 device
- [ ] Create desktop duplication object

**Checklist:**
```rust
struct ScreenCapture {a
    // Add these fields:
    - [ ] dxgi_factory: IDXGIFactory1
    - [ ] adapter: IDXGIAdapter1
    - [ ] device: ID3D11Device
    - [ ] context: ID3D11DeviceContext
    - [ ] output: IDXGIOutput1
    - [ ] duplication: IDXGIOutputDuplication
}

// Implement these methods:
- [ ] fn new() -> Result<Self>
- [ ] fn initialize_dxgi() -> Result<()>
- [ ] fn create_device() -> Result<ID3D11Device>
- [ ] fn create_duplication() -> Result<IDXGIOutputDuplication>
```

### TODO 3.2: Frame Capture Loop
File: `src/capture/frame_processor.rs`

- [ ] Implement AcquireNextFrame() loop
- [ ] Handle DXGI_ERROR_WAIT_TIMEOUT (normal)
- [ ] Get frame info (dirty rects, move rects)
- [ ] Get desktop resource (IDXGIResource)
- [ ] Convert to ID3D11Texture2D
- [ ] Map texture to CPU-readable memory
- [ ] Copy pixel data to buffer
- [ ] ReleaseFrame() to free resources
- [ ] Repeat at 30-60 FPS

**Checklist:**
```rust
- [ ] fn capture_frame() -> Result<Frame>
- [ ] fn acquire_next_frame(timeout_ms: u32) -> Result<FrameInfo>
- [ ] fn map_texture_to_cpu(texture: ID3D11Texture2D) -> Result<Vec<u8>>
- [ ] fn convert_bgra_to_rgb(data: &[u8]) -> Vec<u8>
- [ ] fn release_frame() -> Result<()>
```

### TODO 3.3: Multi-Monitor Support
File: `src/capture/multi_monitor.rs`

- [ ] Enumerate all outputs (monitors)
- [ ] Create duplication for each monitor
- [ ] Capture all monitors simultaneously
- [ ] Identify which monitor has active window
- [ ] Focus on primary monitor by default
- [ ] Handle monitor hotplug events

**Checklist:**
```rust
- [ ] fn get_monitor_count() -> usize
- [ ] fn create_captures_for_all_monitors() -> Vec<ScreenCapture>
- [ ] fn get_active_monitor() -> usize
- [ ] fn capture_all_monitors() -> Vec<Frame>
```

## Day 7: OCR Integration

### TODO 3.4: Setup Tesseract OCR
File: `src/capture/ocr.rs`

- [ ] Download Tesseract trained data (eng.traineddata)
- [ ] Place in resources/tessdata/ folder
- [ ] Initialize Tesseract API
- [ ] Set page segmentation mode (PSM_AUTO)
- [ ] Set whitelist characters if needed
- [ ] Test on sample image

**Checklist:**
```rust
struct OCREngine {
    - [ ] tesseract: Tesseract
}

- [ ] fn new(tessdata_path: &str) -> Result<Self>
- [ ] fn extract_text(image: &Image) -> Result<String>
- [ ] fn set_whitelist(chars: &str) -> Result<()>
- [ ] fn set_page_seg_mode(mode: PageSegMode) -> Result<()>
```

### TODO 3.5: Image Preprocessing
File: `src/capture/image_processing.rs`

- [ ] Convert frame to grayscale
- [ ] Apply contrast enhancement
- [ ] Denoise image (Gaussian blur)
- [ ] Sharpen text regions
- [ ] Threshold (make text black on white)
- [ ] Resize if needed (OCR works best at 300 DPI)

**Checklist:**
```rust
- [ ] fn preprocess_for_ocr(image: &Image) -> Image
- [ ] fn to_grayscale(image: &Image) -> Image
- [ ] fn enhance_contrast(image: &Image) -> Image
- [ ] fn denoise(image: &Image) -> Image
- [ ] fn threshold(image: &Image, threshold: u8) -> Image
```

### TODO 3.6: Question Detection
File: `src/capture/question_detector.rs`

- [ ] Detect regions with text (contour detection)
- [ ] Identify question areas (larger text blocks)
- [ ] Filter out UI elements (buttons, menus)
- [ ] Track question regions across frames
- [ ] Trigger OCR only when new question appears
- [ ] Cache processed questions

**Checklist:**
```rust
- [ ] fn detect_text_regions(image: &Image) -> Vec<Rect>
- [ ] fn find_question_areas(regions: &[Rect]) -> Vec<Rect>
- [ ] fn filter_ui_elements(regions: &[Rect]) -> Vec<Rect>
- [ ] fn has_new_question(current: &Image, previous: &Image) -> bool
```

## Day 8: Performance Optimization

### TODO 3.7: Async Capture Pipeline
File: `src/capture/async_pipeline.rs`

- [ ] Create capture thread (tokio async task)
- [ ] Create processing queue (mpsc channel)
- [ ] Capture frames at 60 FPS
- [ ] Process frames at 10 FPS (skip most)
- [ ] Only OCR when question region changes
- [ ] Measure and log frame times

**Checklist:**
```rust
- [ ] fn start_capture_loop() -> tokio::task::JoinHandle<()>
- [ ] fn create_frame_queue() -> (Sender<Frame>, Receiver<Frame>)
- [ ] fn should_process_frame(frame_num: u64) -> bool
- [ ] fn measure_frame_time() -> Duration
```

### TODO 3.8: Memory Management
File: `src/capture/memory_pool.rs`

- [ ] Create frame buffer pool (reuse allocations)
- [ ] Preallocate 10 frame buffers
- [ ] Recycle buffers instead of allocating new
- [ ] Monitor memory usage
- [ ] Free old frames if memory exceeds limit
- [ ] Target: <200 MB memory usage

**Checklist:**
```rust
- [ ] struct FramePool { buffers: Vec<Vec<u8>> }
- [ ] fn allocate_buffers(count: usize, size: usize) -> FramePool
- [ ] fn get_buffer(&mut self) -> Vec<u8>
- [ ] fn return_buffer(&mut self, buffer: Vec<u8>)
- [ ] fn get_memory_usage() -> usize
```

---

# 📋 PHASE 4: AI INTEGRATION (Days 9-12)

## Day 9: Local AI Setup

### TODO 4.1: Download and Setup Llama Model
- [ ] Download Llama 3.1 8B model (quantized Q4_0)
- [ ] URL: https://huggingface.co/TheBloke/Llama-3.1-8B-Instruct-GGUF
- [ ] File: `llama-3.1-8b-instruct.Q4_0.gguf` (~4.3 GB)
- [ ] Place in `resources/models/` directory
- [ ] Verify file integrity (checksum)

### TODO 4.2: Integrate llama.cpp
File: `Cargo.toml` - Add dependency:
```toml
llama-cpp-rs = "0.2"
```

File: `src/ai/local_llm.rs`

- [ ] Load model from file at startup
- [ ] Configure context size (2048 tokens)
- [ ] Set temperature (0.7 for balanced creativity)
- [ ] Set max tokens (512 for answers)
- [ ] Preload model into memory
- [ ] Test basic inference ("What is 2+2?")

**Checklist:**
```rust
struct LocalLLM {
    - [ ] model: LlamaModel
    - [ ] context: LlamaContext
    - [ ] params: LlamaParams
}

- [ ] fn load_model(path: &Path) -> Result<Self>
- [ ] fn generate(prompt: &str) -> Result<String>
- [ ] fn set_temperature(temp: f32)
- [ ] fn set_max_tokens(max: usize)
```

### TODO 4.3: Prompt Engineering
File: `src/ai/prompts.rs`

- [ ] Create system prompt template
- [ ] Format: "You are a helpful assistant answering exam questions concisely"
- [ ] Add few-shot examples for better accuracy
- [ ] Handle different question types:
  - [ ] Multiple choice (return only letter)
  - [ ] Short answer (2-3 sentences)
  - [ ] Code questions (return code only)
  - [ ] Math problems (show work briefly)

**Checklist:**
```rust
- [ ] fn build_prompt(question: &str, context: &str) -> String
- [ ] fn format_mcq_prompt(question: &str, options: &[String]) -> String
- [ ] fn format_code_prompt(question: &str) -> String
- [ ] fn extract_answer_from_response(response: &str) -> String
```

## Day 10: Cloud AI Fallback

### TODO 4.4: Encrypted API Client
File: `src/ai/cloud_api.rs`

- [ ] Choose provider: Anthropic Claude OR OpenAI GPT
- [ ] Store API key in encrypted config file
- [ ] Implement HTTPS client with reqwest
- [ ] Add retry logic (3 attempts)
- [ ] Timeout: 10 seconds
- [ ] Error handling for rate limits

**Checklist:**
```rust
struct CloudAI {
    - [ ] client: reqwest::Client
    - [ ] api_key: String
    - [ ] endpoint: String
}

- [ ] fn new(api_key: String) -> Self
- [ ] async fn send_query(prompt: &str) -> Result<String>
- [ ] async fn send_with_retry(prompt: &str, retries: usize) -> Result<String>
- [ ] fn handle_rate_limit() -> Duration // backoff time
```

### TODO 4.5: Network Stealth (Domain Fronting)
File: `src/network/stealth_client.rs`

- [ ] Configure SNI header: Use CDN domain (cloudflare.com)
- [ ] Set Host header: Actual API endpoint
- [ ] Add TLS 1.3 with ESNI if available
- [ ] Mimic user-agent of legitimate browser
- [ ] Random timing between requests
- [ ] Traffic shaping: Look like Zoom/Teams patterns

**Checklist:**
```rust
- [ ] fn create_stealth_client() -> reqwest::Client
- [ ] fn set_custom_sni(domain: &str) -> Result<()>
- [ ] fn set_headers() -> HeaderMap
- [ ] fn add_random_delay() -> Duration
- [ ] fn shape_traffic_pattern(data: &[u8]) -> Vec<u8>
```

### TODO 4.6: Traffic Encryption
File: `src/network/encryption.rs`

- [ ] Implement AES-256-GCM encryption
- [ ] Generate random key per session
- [ ] Encrypt all API payloads
- [ ] Add nonce/IV for each message
- [ ] Decrypt responses
- [ ] Zeroize keys after use

**Checklist:**
```rust
- [ ] fn generate_key() -> [u8; 32]
- [ ] fn encrypt_payload(data: &[u8], key: &[u8]) -> Vec<u8>
- [ ] fn decrypt_payload(data: &[u8], key: &[u8]) -> Result<Vec<u8>>
- [ ] fn zeroize_key(key: &mut [u8])
```

## Day 11: Answer Processing

### TODO 4.7: Question Classification
File: `src/ai/classifier.rs`

- [ ] Detect question type from text
- [ ] MCQ: Contains A) B) C) D)
- [ ] Code: Contains ```code``` or specific keywords
- [ ] Math: Contains numbers, equations, symbols
- [ ] Essay: Long form question
- [ ] Route to appropriate AI prompt

**Checklist:**
```rust
enum QuestionType {
    MultipleChoice, ShortAnswer, Code, Math, Essay
}

- [ ] fn classify_question(text: &str) -> QuestionType
- [ ] fn extract_mcq_options(text: &str) -> Vec<String>
- [ ] fn extract_code_context(text: &str) -> Option<String>
```

### TODO 4.8: Answer Formatting
File: `src/ai/formatter.rs`

- [ ] Clean AI response (remove markdown, extra text)
- [ ] Extract just the answer
- [ ] Format for display:
  - [ ] MCQ: Bold letter only
  - [ ] Code: Syntax highlight
  - [ ] Math: Format equations nicely
- [ ] Limit length (fit in overlay)

**Checklist:**
```rust
- [ ] fn clean_response(text: &str) -> String
- [ ] fn extract_mcq_answer(text: &str) -> Option<char>
- [ ] fn format_code(code: &str) -> String
- [ ] fn truncate_if_needed(text: &str, max_len: usize) -> String
```

## Day 12: Caching & Performance

### TODO 4.9: Answer Caching
File: `src/ai/cache.rs`

- [ ] Hash question text (SHA-256)
- [ ] Store in HashMap<Hash, Answer>
- [ ] Check cache before calling AI
- [ ] Persist cache to disk (JSON file)
- [ ] Load cache on startup
- [ ] Limit cache size (max 1000 entries)

**Checklist:**
```rust
struct AnswerCache {
    - [ ] cache: HashMap<String, CachedAnswer>
    - [ ] max_entries: usize
}

- [ ] fn hash_question(text: &str) -> String
- [ ] fn get(&self, question: &str) -> Option<&CachedAnswer>
- [ ] fn insert(&mut self, question: &str, answer: String)
- [ ] fn save_to_disk(&self, path: &Path) -> Result<()>
- [ ] fn load_from_disk(path: &Path) -> Result<Self>
```

---

# 📋 PHASE 5: ANSWER DISPLAY (Days 13-14)

## Day 13: Transparent Overlay

### TODO 5.1: Create Overlay Window
File: `src/overlay/window.rs`

- [ ] Create layered window (WS_EX_LAYERED)
- [ ] Make transparent (SetLayeredWindowAttributes)
- [ ] Set always on top (WS_EX_TOPMOST)
- [ ] Make click-through (WS_EX_TRANSPARENT)
- [ ] No taskbar entry (WS_EX_TOOLWINDOW)
- [ ] Position: Bottom-right corner
- [ ] Size: 300x200 pixels

**Checklist:**
```rust
struct Overlay {
    - [ ] hwnd: HWND
    - [ ] position: (i32, i32)
    - [ ] size: (u32, u32)
}

- [ ] fn create_window() -> Result<HWND>
- [ ] fn set_transparency(alpha: u8) -> Result<()>
- [ ] fn set_always_on_top() -> Result<()>
- [ ] fn set_click_through() -> Result<()>
- [ ] fn hide_from_taskbar() -> Result<()>
```

### TODO 5.2: Render with Dear ImGui
File: `src/overlay/renderer.rs`

- [ ] Initialize Dear ImGui context
- [ ] Create render loop (30 FPS)
- [ ] Draw background (semi-transparent black)
- [ ] Render text (answer)
- [ ] Style: Green text, monospace font
- [ ] Add fade in/out animations
- [ ] Auto-hide after 30 seconds

**Checklist:**
```rust
- [ ] fn init_imgui() -> Result<imgui::Context>
- [ ] fn render_frame(ui: &Ui, answer: &str)
- [ ] fn draw_background(ui: &Ui, alpha: f32)
- [ ] fn draw_text(ui: &Ui, text: &str, color: [f32; 4])
- [ ] fn animate_fade(elapsed: Duration) -> f32
```

### TODO 5.3: Display Control
File: `src/overlay/controller.rs`

- [ ] Show overlay when answer ready
- [ ] Hide after timeout or manual dismiss
- [ ] Hotkey to toggle: Ctrl+Shift+H
- [ ] Multiple answers: Show queue
- [ ] Position adjustable (corners, center)
- [ ] Font size adjustable

**Checklist:**
```rust
- [ ] fn show_answer(text: String)
- [ ] fn hide_overlay()
- [ ] fn register_hotkey(key: VirtualKey) -> Result<()>
- [ ] fn handle_hotkey_press()
- [ ] fn set_position(corner: Corner)
```

## Day 14: Alternative Display Methods

### TODO 5.4: System Tray Icon
File: `src/overlay/tray_icon.rs`

- [ ] Create system tray icon
- [ ] Icon: Generic system icon (🔒 or 🔔)
- [ ] Tooltip: "Windows Security Monitor"
- [ ] Right-click menu:
  - [ ] Show last answer
  - [ ] Settings
  - [ ] Exit
- [ ] Balloon notification for new answers

**Checklist:**
```rust
- [ ] fn create_tray_icon() -> Result<()>
- [ ] fn set_tooltip(text: &str)
- [ ] fn show_context_menu() -> Result<()>
- [ ] fn show_balloon_notification(title: &str, text: &str)
```

### TODO 5.5: Second Device Sync
File: `src/network/device_sync.rs`

- [ ] Create local WebSocket server
- [ ] Listen on: localhost:8765
- [ ] Accept connections from phone/tablet
- [ ] Send answers as JSON
- [ ] Encrypt with shared key
- [ ] Web interface for second device

**Checklist:**
```rust
- [ ] fn start_websocket_server(port: u16) -> Result<()>
- [ ] async fn handle_client(stream: TcpStream)
- [ ] fn send_answer_to_clients(answer: &str) -> Result<()>
- [ ] fn create_web_interface() -> String // HTML page
```

---

# 📋 PHASE 6: TESTING & HARDENING (Days 15-17)

## Day 15: Stealth Testing

### TODO 6.1: Test Against Detection Tools
- [ ] **Task Manager**: Open and verify process not visible
- [ ] **Resource Monitor**: Check if hidden in processes
- [ ] **Process Explorer** (Sysinternals): Look for your app
- [ ] **Process Hacker**: Advanced process viewer test
- [ ] **TCPView**: Verify no suspicious network connections
- [ ] Document: What's visible, what's not

**Test Checklist:**
- [ ] Process list: ✓ Not visible / ✗ Visible
- [ ] Windows list: ✓ No window / ✗ Window shown
- [ ] Network: ✓ No traffic / ✓ Looks legitimate
- [ ] Startup: ✓ Survives reboot / ✗ Doesn't start
- [ ] Service: ✓ Appears legitimate / ✗ Suspicious

### TODO 6.2: Test Against Antivirus
- [ ] **Windows Defender**: Full scan
- [ ] **Kaspersky Free** (download trial): Full scan
- [ ] **Malwarebytes** (download free): Scan
- [ ] **VirusTotal** (upload binary): Check detection rate
- [ ] If detected: Implement additional obfuscation

**Test Results:**
- [ ] Windows Defender: ✓ Pass / ✗ Detected
- [ ] Kaspersky: ✓ Pass / ✗ Detected
- [ ] Malwarebytes: ✓ Pass / ✗ Detected
- [ ] VirusTotal: X/70 detections (goal: <5)

### TODO 6.3: Performance Testing
- [ ] Measure CPU usage (Task Manager Performance tab)
  - [ ] Idle: Target <5%
  - [ ] Capturing: Target <15%
  - [ ] AI processing: Target <40% (local) or <5% (cloud)
- [ ] Measure RAM usage
  - [ ] Idle: Target <50 MB
  - [ ] Active: Target <500 MB
- [ ] Measure disk usage
  - [ ] Installation: <200 MB
- [ ] Measure network (if cloud AI)
  - [ ] Per query: <100 KB
  - [ ] Per hour: <10 MB

## Day 16: Proctoring System Testing

### TODO 6.4: Test Against Respondus LockDown Browser
- [ ] Download and install Respondus
- [ ] Take sample test in Respondus
- [ ] Verify your app runs simultaneously
- [ ] Check if Respondus detects anything
- [ ] Test screen capture works
- [ ] Test AI answers display correctly
- [ ] Document: What works, what's blocked

**Test Checklist:**
- [ ] App starts before Respondus: ✓/✗
- [ ] App runs during test: ✓/✗
- [ ] Screen capture functional: ✓/✗
- [ ] Respondus shows warning: ✓/✗
- [ ] Test submittable: ✓/✗

### TODO 6.5: Test Against ProctorU
- [ ] Sign up for ProctorU demo/trial
- [ ] Installcargo run