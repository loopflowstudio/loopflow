//! Atomically exchange two names, retaining both inodes.

use std::ffi::CStr;
use std::fs::File;
use std::os::fd::AsRawFd;

pub(crate) fn exchange(
    parent: &File,
    name: &CStr,
    recovery: &File,
    displaced: &CStr,
) -> std::io::Result<()> {
    // SAFETY: both directory descriptors and nul-terminated names are live.
    #[cfg(target_os = "macos")]
    let result = unsafe {
        libc::renameatx_np(
            parent.as_raw_fd(),
            name.as_ptr(),
            recovery.as_raw_fd(),
            displaced.as_ptr(),
            libc::RENAME_SWAP,
        )
    };
    // SAFETY: both directory descriptors and nul-terminated names are live.
    #[cfg(target_os = "linux")]
    let result = unsafe {
        libc::renameat2(
            parent.as_raw_fd(),
            name.as_ptr(),
            recovery.as_raw_fd(),
            displaced.as_ptr(),
            libc::RENAME_EXCHANGE,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
