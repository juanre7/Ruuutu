//! Console attachment for a GUI-subsystem binary.
//!
//! `src/main.rs` is built with `#![windows_subsystem = "windows"]`, so double-clicking
//! `ruuutu.exe` never flashes a console window: it goes straight to the tray. The price is
//! that a GUI binary starts with no console at all, so running it from cmd/PowerShell would
//! swallow every `println!` too.
//!
//! `attach_parent_console()` buys that output back: if the process was launched from a
//! terminal, `AttachConsole(ATTACH_PARENT_PROCESS)` borrows the parent's console and the
//! standard handles are pointed at it. Launched from Explorer there is no parent console,
//! the call fails, and everything stays silent as intended.
//!
//! Must run before the first `println!`: Rust's `std::io::stdout` caches the handle returned
//! by `GetStdHandle` on first use.

#[cfg(windows)]
pub fn attach_parent_console() {
    use windows_sys::Win32::Foundation::{GENERIC_WRITE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Console::{
        AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE,
        STD_HANDLE, STD_OUTPUT_HANDLE,
    };

    unsafe {
        // Fails with ERROR_INVALID_HANDLE when there is no parent console (Explorer,
        // autostart, scheduled task). That is the double-click path: stay silent.
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return;
        }

        // "CONOUT$" as UTF-16, NUL terminated.
        let conout: [u16; 8] = [
            b'C' as u16, b'O' as u16, b'N' as u16, b'O' as u16, b'U' as u16, b'T' as u16,
            b'$' as u16, 0,
        ];

        for std_handle in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            // Only fill in handles the parent did not give us. `ruuutu.exe > log.txt` hands
            // over a real file handle; overwriting it would break the redirection.
            let current = GetStdHandle(std_handle as STD_HANDLE);
            if current != 0 && current != INVALID_HANDLE_VALUE {
                continue;
            }

            let h = CreateFileW(
                conout.as_ptr(),
                GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                0,
            );
            if h != INVALID_HANDLE_VALUE {
                SetStdHandle(std_handle as STD_HANDLE, h);
            }
        }
    }
}

#[cfg(not(windows))]
pub fn attach_parent_console() {}
