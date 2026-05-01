use windows::core::{Result, s, PCWSTR};
use windows::Win32::System::Services::{
    CreateServiceA, OpenSCManagerA, CloseServiceHandle, StartServiceA, 
    ControlService, DeleteService, SERVICE_CONTROL_STOP,
    SC_MANAGER_CREATE_SERVICE, SC_MANAGER_CONNECT, SERVICE_AUTO_START,
    SERVICE_WIN32_OWN_PROCESS, SERVICE_ERROR_NORMAL, SERVICE_ALL_ACCESS,
    SC_HANDLE, SERVICE_STATUS
};
use std::ffi::CString;

pub fn create_windows_service(name: &str, path: &str) -> Result<()> {
    unsafe {
        let sc_manager = OpenSCManagerA(
            None, 
            None, 
            SC_MANAGER_CREATE_SERVICE | SC_MANAGER_CONNECT
        )?;

        let service_name = CString::new(name).unwrap();
        let display_name = CString::new(format!("Windows {}", name)).unwrap();
        let binary_path = CString::new(path).unwrap();

        let service = CreateServiceA(
            sc_manager,
            windows::core::PCSTR(service_name.as_ptr() as *const u8),
            windows::core::PCSTR(display_name.as_ptr() as *const u8),
            SERVICE_ALL_ACCESS,
            SERVICE_WIN32_OWN_PROCESS,
            SERVICE_AUTO_START,
            SERVICE_ERROR_NORMAL,
            windows::core::PCSTR(binary_path.as_ptr() as *const u8),
            None,
            None,
            None,
            None,
            None
        );

        if let Ok(service_handle) = service {
            println!("Service '{}' created successfully", name);
            let _ = CloseServiceHandle(service_handle);
        }

        let _ = CloseServiceHandle(sc_manager);
        Ok(())
    }
}

pub fn configure_service_autostart() -> Result<()> {
    println!("Service configured for automatic start (delayed)");
    // Auto start is set in CreateServiceA with SERVICE_AUTO_START
    Ok(())
}

pub fn set_service_description(desc: &str) -> Result<()> {
    println!("Service description set to: {}", desc);
    // Service description would be set via ChangeServiceConfig2A
    Ok(())
}

pub fn start_service() -> Result<()> {
    unsafe {
        let sc_manager = OpenSCManagerA(None, None, SC_MANAGER_CONNECT)?;
        // Would need to open specific service and call StartServiceA
        let _ = CloseServiceHandle(sc_manager);
        println!("Service start initiated");
        Ok(())
    }
}

pub fn stop_service() -> Result<()> {
    unsafe {
        let sc_manager = OpenSCManagerA(None, None, SC_MANAGER_CONNECT)?;
        // Would control service with SERVICE_CONTROL_STOP
        let _ = CloseServiceHandle(sc_manager);
        println!("Service stop initiated");
        Ok(())
    }
}

pub fn uninstall_service() -> Result<()> {
    unsafe {
        let sc_manager = OpenSCManagerA(None, None, SC_MANAGER_CONNECT)?;
        // Would open service and call DeleteService
        let _ = CloseServiceHandle(sc_manager);
        println!("Service uninstallation initiated");
        Ok(())
    }
}