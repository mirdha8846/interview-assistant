use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Threading::*;
use windows::Win32::System::Diagnostics::ToolHelp::*;
use std::mem;
use std::ffi::CStr;

pub fn launch_with_parent(path: &str, parent_name: &str) -> Result<()> {
    unsafe {
        // 1. Find parent PID
        let parent_pid = find_process_id(parent_name)?;
        println!("Found parent {} with PID: {}", parent_name, parent_pid);

        // 2. Open parent process
        // PROCESS_CREATE_PROCESS (0x0080) is required for PPID spoofing
        let parent_handle = OpenProcess(
            PROCESS_CREATE_PROCESS, 
            FALSE,
            parent_pid
        )?;

        // In windows-rs 0.52, OpenProcess returns Result<HANDLE>. If we are here, it succeeded.
        // But we should check if handle is valid just in case, though Result handles it.
        
        // 3. Initialize Attribute List
        let mut size: usize = 0;
        // First call to get size. Pass null pointer wrapped in struct.
        let null_list = LPPROC_THREAD_ATTRIBUTE_LIST(std::ptr::null_mut());
        let _ = InitializeProcThreadAttributeList(null_list, 1, 0, &mut size); 
        
        let mut buffer = vec![0u8; size];
        let lp_attribute_list = LPPROC_THREAD_ATTRIBUTE_LIST(buffer.as_mut_ptr() as *mut _);

        InitializeProcThreadAttributeList(lp_attribute_list, 1, 0, &mut size)?;

        // 4. Update Attribute with Parent Handle
        // PROC_THREAD_ATTRIBUTE_PARENT_PROCESS is the attribute we need
        let mut parent_handle_ptr = parent_handle;
        UpdateProcThreadAttribute(
            lp_attribute_list,
            0,
            PROC_THREAD_ATTRIBUTE_PARENT_PROCESS as usize,
            Some(&mut parent_handle_ptr as *mut _ as *mut _),
            mem::size_of::<HANDLE>(),
            None,
            None
        )?;

        // 5. Create Process
        let mut si = STARTUPINFOEXA::default();
        si.StartupInfo.cb = mem::size_of::<STARTUPINFOEXA>() as u32;
        si.lpAttributeList = lp_attribute_list;

        let mut pi = PROCESS_INFORMATION::default();
        let command_line = format!("{}\0", path);
        let mut cmd_buf = command_line.into_bytes();

        let result = CreateProcessA(
            None,
            PSTR(cmd_buf.as_mut_ptr()),
            None,
            None,
            FALSE,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW, // Flags
            None,
            None,
            &si.StartupInfo,
            &mut pi
        );

        // Cleanup
        DeleteProcThreadAttributeList(lp_attribute_list);
        let _ = CloseHandle(parent_handle);

        if result.is_ok() {
            println!("Successfully launched {} with parent {}", path, parent_name);
            let _ = CloseHandle(pi.hProcess);
            let _ = CloseHandle(pi.hThread);
            Ok(())
        } else {
            Err(Error::from_win32())
        }
    }
}

fn find_process_id(name: &str) -> Result<u32> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)?;
        let mut entry = PROCESSENTRY32::default();
        entry.dwSize = mem::size_of::<PROCESSENTRY32>() as u32;

        if Process32First(snapshot, &mut entry).is_ok() {
            loop {
                let current_name = CStr::from_ptr(entry.szExeFile.as_ptr() as *const i8);
                if current_name.to_string_lossy().eq_ignore_ascii_case(name) {
                    let _ = CloseHandle(snapshot);
                    return Ok(entry.th32ProcessID);
                }

                if !Process32Next(snapshot, &mut entry).is_ok() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
        Err(Error::from(ERROR_NOT_FOUND)) 
    }
}
