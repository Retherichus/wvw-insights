use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use winapi::shared::minwindef::HMODULE;
use winapi::um::libloaderapi::GetModuleFileNameW;

/// Attempts to sync the log directory setting with ArcDPS configuration
pub fn sync_with_arcdps() -> Result<String, String> {
    // Get GW2 executable path
    let mut buffer = [0u16; 4096];
    let len = unsafe {
        GetModuleFileNameW(
            std::ptr::null_mut() as HMODULE,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
        )
    };

    if len == 0 {
        return Err("Unable to locate Guild Wars 2 directory".to_string());
    }

    let exe_path = OsString::from_wide(&buffer[..len as usize]);
    let exe_path = PathBuf::from(exe_path);
    let gw2_dir = exe_path
        .parent()
        .ok_or("Unable to determine GW2 directory")?;

    // Try multiple possible locations for arcdps.ini
    let possible_paths = [
        gw2_dir.join("arcdps.ini"),
        gw2_dir.join("addons").join("arcdps.ini"),
        gw2_dir.join("addons").join("arcdps").join("arcdps.ini"),
    ];

    for ini_path in &possible_paths {
        if ini_path.exists() {
            // Read the file
            if let Ok(contents) = std::fs::read_to_string(ini_path) {
                // Look for boss_encounter_path line
                for line in contents.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("boss_encounter_path=") {
                        let path = trimmed
                            .trim_start_matches("boss_encounter_path=")
                            .trim();
                        if !path.is_empty() {
                            log::info!("Found ArcDPS log path: {}", path);
                            return Ok(path.to_string());
                        }
                    }
                }
            }
        }
    }

    Err("⚠ Unable to locate arcdps.ini or boss_encounter_path setting".to_string())
}