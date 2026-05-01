use windows::Win32::System::Diagnostics::Debug::{IsDebuggerPresent, CheckRemoteDebuggerPresent};
use windows::Win32::System::Threading::GetCurrentProcess;
use windows::Win32::Foundation::BOOL;

pub fn is_debugger_present() -> bool {
    unsafe {
        IsDebuggerPresent().as_bool()
    }
}

pub fn check_remote_debugger() -> bool {
    let mut is_remote_debugger_present = BOOL(0);
    unsafe {
        let _ = CheckRemoteDebuggerPresent(GetCurrentProcess(), &mut is_remote_debugger_present);
    }
    is_remote_debugger_present.as_bool()
}

pub fn ensure_no_debug() {
    if is_debugger_present() || check_remote_debugger() {
        // In a real scenario, we might exit or corrupt memory.
        // For now, we just log it.
        println!("DEBUGGER DETECTED! Initiating evasion protocols...");
        // std::process::exit(1); // Uncomment to enable strict exit
    }
}
