use std::ffi::{CStr, CString};
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::path::{Component, Path};

/// Pin each directory before opening the next component. Retargeting a symlink
/// during a search or save cannot redirect the open outside the checkout.
pub(super) fn parent_directory(root: &File, relative: &Path) -> io::Result<(File, CString)> {
    let mut directory = root.try_clone()?;
    let mut components = relative.components().peekable();
    while let Some(Component::Normal(name)) = components.next() {
        let name = CString::new(name.as_encoded_bytes())?;
        if components.peek().is_none() {
            return Ok((directory, name));
        }
        directory = open_at(&directory, &name, libc::O_DIRECTORY)?;
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "Expected a checkout-relative file",
    ))
}

pub(super) fn open_file(parent: &File, name: &CStr) -> io::Result<File> {
    open_at(parent, name, libc::O_NONBLOCK)
}

fn open_at(parent: &File, name: &CStr, flags: libc::c_int) -> io::Result<File> {
    // SAFETY: parent and name are live, and openat returns a fresh owned descriptor.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | flags,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the successful openat returned a fresh descriptor owned by this File.
    Ok(unsafe { File::from_raw_fd(fd) })
}
