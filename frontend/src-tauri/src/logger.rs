use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use lazy_static::lazy_static;
use chrono::Local;

// =============================================================================
// 📝 LOGGER CONFIGURATION
// =============================================================================

/// Track initialization status
static LOGGER_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Count write errors (for debugging, don't spam console)
static WRITE_ERROR_COUNT: AtomicU32 = AtomicU32::new(0);

/// Max errors before we stop trying to write
const MAX_WRITE_ERRORS: u32 = 10;

lazy_static! {
    static ref LOG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);
}

/// Initialize the logger with a file path.
/// Falls back gracefully if file cannot be opened.
pub fn init(filename: &str) {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(filename);

    match file {
        Ok(f) => {
            match LOG_FILE.lock() {
                Ok(mut log_file) => {
                    *log_file = Some(f);
                    LOGGER_INITIALIZED.store(true, Ordering::SeqCst);
                    println!("✅ Logger initialized: {}", filename);
                }
                Err(e) => {
                    eprintln!("❌ Logger mutex poisoned: {:?}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to initialize logger file '{}': {}", filename, e);
            eprintln!("   Logs will only appear in console.");
            // Logger still works, just without file output
        }
    }
}

/// Check if logger was successfully initialized with a file
pub fn is_file_logging_enabled() -> bool {
    LOGGER_INITIALIZED.load(Ordering::SeqCst)
}

pub fn log(level: &str, message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let log_entry = format!("[{}] [{}] {}\n", timestamp, level, message);

    // Print to console always (for dev mode and fallback)
    print!("{}", log_entry);

    // Check if we've had too many write errors - stop trying
    if WRITE_ERROR_COUNT.load(Ordering::Relaxed) >= MAX_WRITE_ERRORS {
        return; // File logging disabled due to repeated errors
    }

    // Write to file - use try_lock to never block
    if let Ok(mut file_guard) = LOG_FILE.try_lock() {
        if let Some(file) = file_guard.as_mut() {
            if let Err(_e) = file.write_all(log_entry.as_bytes()) {
                let error_count = WRITE_ERROR_COUNT.fetch_add(1, Ordering::Relaxed);
                if error_count == 0 {
                    // Only print first error to avoid spam
                    eprintln!("⚠️  Logger write error - file logging may be degraded");
                } else if error_count + 1 >= MAX_WRITE_ERRORS {
                    eprintln!("❌ Too many logger errors - file logging disabled");
                }
            }
        }
    }
    // If lock is contended or unavailable, just skip file write (console already printed)
}

/// Force flush the log file (useful before crash/exit)
pub fn flush() {
    if let Ok(mut file_guard) = LOG_FILE.lock() {
        if let Some(file) = file_guard.as_mut() {
            let _ = file.flush();
        }
    }
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        crate::logger::log("INFO", &format!($($arg)*))
    }
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        crate::logger::log("ERROR", &format!($($arg)*))
    }
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        crate::logger::log("WARN", &format!($($arg)*))
    }
}
