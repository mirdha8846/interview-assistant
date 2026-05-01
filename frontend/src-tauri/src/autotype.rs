use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use rand::Rng;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VIRTUAL_KEY,
    VK_RETURN, VK_TAB, VK_DELETE
};
use std::mem::size_of;

static IS_TYPING: AtomicBool = AtomicBool::new(false);
static AUTO_BRACKET_MODE: AtomicBool = AtomicBool::new(true); // Default ON

pub fn is_typing() -> bool {
    IS_TYPING.load(Ordering::SeqCst)
}

pub fn is_auto_bracket_mode() -> bool {
    AUTO_BRACKET_MODE.load(Ordering::SeqCst)
}

pub fn toggle_auto_bracket_mode() -> bool {
    let current = AUTO_BRACKET_MODE.load(Ordering::SeqCst);
    let new_value = !current;
    AUTO_BRACKET_MODE.store(new_value, Ordering::SeqCst);
    crate::log_info!("Auto-bracket mode: {}", if new_value { "ON" } else { "OFF" });
    new_value
}

pub fn stop_typing() {
    IS_TYPING.store(false, Ordering::SeqCst);
    crate::log_info!("Auto-type stopped by user request.");
}

pub fn start_typing(text: String) {
    if is_typing() {
        crate::log_info!("Already typing, ignoring request.");
        return;
    }

    IS_TYPING.store(true, Ordering::SeqCst);
    
    // Sanitize the text before typing
    let clean_text = sanitize_code(&text);
    
    thread::spawn(move || {
        crate::log_info!("Starting auto-type for {} characters", clean_text.len());
        let mut rng = rand::thread_rng();

        // Split into lines to handle indentation smartly
        let lines: Vec<&str> = clean_text.lines().collect();
        let total_lines = lines.len();

        for (i, line) in lines.iter().enumerate() {
            if !IS_TYPING.load(Ordering::SeqCst) { break; }

            // Smart Trim: Remove leading whitespace because editors (vscode/leetcode)
            // usually auto-indent when you press Enter. Typing the spaces again causes
            // "diagonal" staircase effect.
            let trimmed_line = line.trim_start();

            for c in trimmed_line.chars() {
                if !IS_TYPING.load(Ordering::SeqCst) { break; }

                // Simulate human typing speed
                let delay = rng.gen_range(30..120); 
                thread::sleep(Duration::from_millis(delay));

                // Occasionally pause longer (thinking pause)
                if rng.gen_bool(0.05) {
                   thread::sleep(Duration::from_millis(rng.gen_range(300..800)));
                }
                
                unsafe {
                    match c {
                        '\t' => send_key(VK_TAB),
                        // For characters that editors auto-close, press Delete after typing
                        // to remove the auto-inserted closing character (only if mode is ON)
                        '{' | '(' | '[' | '"' | '\'' if is_auto_bracket_mode() => {
                            send_unicode_char(c);
                            thread::sleep(Duration::from_millis(10)); // Small delay for editor to react
                            send_key(VK_DELETE); // Remove auto-inserted closing char
                        }
                        _ => send_unicode_char(c),
                    }
                }
            }

            // Press Enter at the end of the line (except possibly the very last line if desired, 
            // but usually a trailing newline is fine).
            if i < total_lines - 1 {
                let delay = rng.gen_range(30..120); 
                thread::sleep(Duration::from_millis(delay));
                unsafe { send_key(VK_RETURN); }
            }
        }

        IS_TYPING.store(false, Ordering::SeqCst);
        crate::log_info!("Auto-type completed.");
    });
}

fn sanitize_code(text: &str) -> String {
    let mut lines: Vec<&str> = text.lines().collect();
    
    // 1. Remove Markdown code fences (```)
    lines.retain(|line| !line.trim().starts_with("```"));

    // 2. Join back to string
    let text_no_fences = lines.join("\n");
    
    // 3. Negative Balance Cutoff
    // The moment we see a closing brace that has no matching opening brace, 
    // we assume everything from that point onwards is garbage/hallucination.
    let mut balance = 0;
    let mut cutoff_index = text_no_fences.len();
    
    for (i, c) in text_no_fences.char_indices() {
        if c == '{' { 
            balance += 1; 
        } else if c == '}' {
            balance -= 1;
            if balance < 0 {
                // Found the first extra closing brace
                // This is the start of the garbage
                cutoff_index = i;
                break;
            }
        }
    }

    let clean_text = &text_no_fences[..cutoff_index];
    clean_text.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_code_removes_fences() {
        let input = "```cpp\nint main() {}\n```";
        let expected = "int main() {}";
        assert_eq!(sanitize_code(input), expected);
    }

    #[test]
    fn test_sanitize_code_balances_braces() {
        let input = "void foo() {\n    return;\n}}\n}";
        // balance logic:
        // { (1)
        // } (0)
        // } (-1) -> Cutoff!
        // Result: "void foo() {\n    return;\n}"
        let result = sanitize_code(input);
        assert!(result.contains("return;"));
        assert_eq!(result.chars().filter(|&c| c == '{').count(), 1);
        assert_eq!(result.chars().filter(|&c| c == '}').count(), 1);
    }
    
    #[test]
    fn test_sanitize_code_handles_complex_garbage() {
        let input = "Class Solution {\n   // code \n};\n        }\n    }\n}";
        // { (1)
        // } (0)
        // } (-1) -> Cutoff
        let result = sanitize_code(input);
        assert_eq!(result.trim(), "Class Solution {\n   // code \n};");
    }

    #[test]
    fn test_sanitize_user_report() {
        let input = r#"Class Solution { 
    public:
    void setZeroes(vector<vector<int>>& matrix) {
        int m = matrix.size(), n = matrix[0].size();
        bool row0 = false, col0 = false;

        for (int i = 0; i < m; i++) {
        }
    }
};
            }
        }
    }"#;
        // Valid block has balanced braces.
        // Garbage starts after };
        let result = sanitize_code(input);
        
        // It should end with }; roughly
        assert!(result.trim().ends_with("};"));
        
        // Count should match
        let result_opens = result.chars().filter(|&c| c == '{').count();
        let result_closes = result.chars().filter(|&c| c == '}').count();
        assert_eq!(result_opens, result_closes, "Braces should be balanced");
    }
}


unsafe fn send_key(vk: VIRTUAL_KEY) {
    let inputs = [
        // Key Down
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk, // Virtual Key
                    wScan: 0,
                    dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0), 
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        // Key Up
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    ];

    SendInput(&inputs, size_of::<INPUT>() as i32);
}

unsafe fn send_unicode_char(c: char) {
    let inputs = [
        // Key Down
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: c as u16,
                    dwFlags: KEYEVENTF_UNICODE, 
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        // Key Up
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: c as u16,
                    dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    ];

    SendInput(&inputs, size_of::<INPUT>() as i32);
}
