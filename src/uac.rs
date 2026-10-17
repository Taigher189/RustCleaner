//! Windows UAC Elevation & Privilege Manager
//! Reconstructed from CCleaner64.exe PE Manifest (Res 24/1/1033 @ 0x2b503b0)
//! and token privilege strings (`SeBackupPrivilege` @ 0x1a09138, `SeDebugPrivilege` @ 0x1a26568).

use std::env;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HWND};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// Returns `true` if the current process is running with elevated Administrator privileges (UAC).
pub fn is_elevated() -> bool {
    unsafe {
        let mut token_handle: HANDLE = ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle) == 0 {
            return false;
        }

        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut return_length: u32 = 0;
        let success = GetTokenInformation(
            token_handle,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut return_length,
        );

        CloseHandle(token_handle);
        success != 0 && elevation.TokenIsElevated != 0
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

/// Relaunches the current executable with the `"runas"` verb to trigger the Windows UAC prompt.
/// Returns `Ok(true)` if the elevated process was started.
pub fn relaunch_as_admin(extra_args: &[String]) -> Result<bool, String> {
    let exe_path = env::current_exe().map_err(|e| format!("Impossibile ottenere il percorso exe: {e}"))?;
    let exe_str = exe_path.to_string_lossy().to_string();

    let args_joined = if extra_args.is_empty() {
        env::args()
            .skip(1)
            .map(|a| format!("\"{}\"", a.replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        extra_args
            .iter()
            .map(|a| format!("\"{}\"", a.replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(" ")
    };

    let verb_w = to_wide("runas");
    let file_w = to_wide(&exe_str);
    let params_w = to_wide(&args_joined);

    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut() as HWND,
            verb_w.as_ptr(),
            file_w.as_ptr(),
            if args_joined.is_empty() {
                ptr::null()
            } else {
                params_w.as_ptr()
            },
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };

    if (result as isize) > 32 {
        Ok(true)
    } else {
        Err(format!(
            "Richiesta UAC annullata o fallita (codice ShellExecuteW: {})",
            result as isize
        ))
    }
}
