//! Read the kernel directly: macOS's setuid ps cannot execute inside an
//! inherited sandbox. Birth identity and command evidence keep the same owner.

use std::io;
use std::mem::{size_of, MaybeUninit};

use super::OsProcess;

#[derive(Debug)]
#[repr(C)]
struct ProcessInfo {
    pid: u32,
    ppid: u32,
    pgid: u32,
    status: u32,
    started_at: i64,
    name: [u8; 17],
}

pub(super) fn read(pid: u32) -> io::Result<Option<OsProcess>> {
    unsafe extern "C" {
        fn loopflow_read_process(pid: u32, output: *mut ProcessInfo) -> i32;
    }
    let mut info = MaybeUninit::<ProcessInfo>::zeroed();
    // SAFETY: the SDK shim writes exactly the matching repr(C) structure and
    // retains no pointer. Only a successful read initializes all its fields.
    match unsafe { loopflow_read_process(pid, info.as_mut_ptr()) } {
        0 => return Ok(None),
        1 => {}
        _ => {
            let error = io::Error::last_os_error();
            return if error.raw_os_error() == Some(libc::ESRCH) {
                Ok(None)
            } else {
                Err(error)
            };
        }
    }
    // SAFETY: the shim returned a complete initialized structure.
    let info = unsafe { info.assume_init() };
    let command = arguments(pid)?.unwrap_or_else(|| {
        String::from_utf8_lossy(&info.name)
            .trim_end_matches('\0')
            .to_string()
    });
    Ok(Some(OsProcess {
        pid: info.pid,
        ppid: info.ppid,
        pgid: info.pgid,
        started_at: info.started_at,
        kernel_state: if info.status == 5 { "Z" } else { "S" }.into(),
        command,
    }))
}

pub(super) fn sample() -> io::Result<Vec<OsProcess>> {
    // PROC_ALL_PIDS = 1. The first call sizes the byte buffer; leave room for
    // processes born between reads, as ps also samples a changing inventory.
    // SAFETY: a null, zero-sized buffer asks libproc for the required size.
    let size = unsafe { libc::proc_listpids(1, 0, std::ptr::null_mut(), 0) };
    if size <= 0 {
        return Err(io::Error::last_os_error());
    }
    let mut pids = vec![0i32; size as usize / size_of::<i32>() + 256];
    // SAFETY: pids is a writable buffer of the supplied byte length.
    let bytes = unsafe {
        libc::proc_listpids(
            1,
            0,
            pids.as_mut_ptr().cast(),
            (pids.len() * size_of::<i32>()) as i32,
        )
    };
    if bytes < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut processes = Vec::new();
    for pid in pids
        .into_iter()
        .take(bytes as usize / size_of::<i32>())
        .filter(|pid| *pid > 0)
    {
        match read(pid as u32)? {
            Some(process) => processes.push(process),
            None => continue,
        }
    }
    Ok(processes)
}

fn arguments(pid: u32) -> io::Result<Option<String>> {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as i32];
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut length = buffer.len();
    // SAFETY: mib and buffer are live input/output arrays. No new value is
    // supplied; this only reads process arguments from the kernel.
    let result = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            buffer.as_mut_ptr().cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 {
        let error = io::Error::last_os_error();
        return if matches!(
            error.raw_os_error(),
            Some(libc::ESRCH | libc::EINVAL | libc::EPERM | libc::EACCES | libc::EIO)
        ) {
            Ok(None)
        } else {
            Err(error)
        };
    }
    buffer.truncate(length);
    Ok(decode_arguments(&buffer))
}

fn decode_arguments(buffer: &[u8]) -> Option<String> {
    let count = i32::from_ne_bytes(buffer.get(..4)?.try_into().ok()?);
    let body = buffer.get(4..)?;
    let executable_end = body.iter().position(|byte| *byte == 0)?;
    let body = body.get(executable_end + 1..)?;
    let start = body.iter().position(|byte| *byte != 0)?;
    // argc is the boundary: bytes after these strings are the environment,
    // which must never become command evidence or reach logs.
    let args = body[start..]
        .split(|byte| *byte == 0)
        .take(usize::try_from(count).ok()?)
        .map(String::from_utf8_lossy)
        .collect::<Vec<_>>();
    Some(args.join(" "))
}

#[cfg(test)]
mod tests {
    use super::decode_arguments;

    #[test]
    fn arguments_exclude_environment() {
        let mut bytes = 3i32.to_ne_bytes().to_vec();
        bytes.extend_from_slice(
            b"/bin/codex\0\0/bin/codex\0app-server\0--listen\0SECRET=never-log\0",
        );
        assert_eq!(
            decode_arguments(&bytes).as_deref(),
            Some("/bin/codex app-server --listen")
        );
    }
}
