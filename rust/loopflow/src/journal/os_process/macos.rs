//! Read the kernel directly: macOS's setuid ps cannot execute inside an
//! inherited sandbox. Birth identity and command evidence keep the same owner.

use std::ffi::CStr;
use std::io;
use std::mem::{size_of, MaybeUninit};
use std::path::PathBuf;

use super::OsProcess;

#[derive(Debug)]
#[repr(C)]
struct ProcessInfo {
    pid: u32,
    ppid: u32,
    pgid: u32,
    status: u32,
    flags: u32,
    started_at: i64,
    name: [u8; 17],
}

impl ProcessInfo {
    fn into_process(self, command: String) -> OsProcess {
        OsProcess {
            pid: self.pid,
            ppid: self.ppid,
            pgid: self.pgid,
            started_at: self.started_at,
            kernel_state: kernel_state(self.status, self.flags).into(),
            command,
        }
    }
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
    let command = arguments(pid)?.unwrap_or_else(|| name(&info.name));
    Ok(Some(info.into_process(command)))
}

/// Members of one process group in a single kernel query, named by `p_comm`.
/// Arguments are never read: they can carry prompt text into error messages.
pub(super) fn group(pgid: u32) -> io::Result<Vec<OsProcess>> {
    unsafe extern "C" {
        fn loopflow_read_group(pgid: u32, output: *mut ProcessInfo, capacity: i32) -> i32;
    }
    let mut capacity = 16;
    loop {
        let mut members = Vec::<ProcessInfo>::with_capacity(capacity);
        // SAFETY: the shim writes at most `capacity` repr(C) structures into
        // the reserved buffer and retains no pointer.
        let count = unsafe { loopflow_read_group(pgid, members.as_mut_ptr(), capacity as i32) };
        let count = usize::try_from(count).map_err(|_| io::Error::last_os_error())?;
        if count > capacity {
            capacity = count + 16;
            continue;
        }
        // SAFETY: the shim initialized the first `count` structures.
        unsafe { members.set_len(count) };
        return Ok(members
            .into_iter()
            .map(|info| {
                let command = name(&info.name);
                info.into_process(command)
            })
            .collect());
    }
}

/// `p_comm` ends at its first NUL; the bytes after it are stale.
fn name(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

// SZOMB awaits its parent's wait. P_WEXIT is set once the kernel starts
// tearing a process down; ps calls that state "dead" (X). Neither runs user code.
const SZOMB: u32 = 5;
const P_WEXIT: u32 = 0x2000;

fn kernel_state(status: u32, flags: u32) -> &'static str {
    if status == SZOMB {
        "Z"
    } else if flags & P_WEXIT != 0 {
        "X"
    } else {
        "S"
    }
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

#[link(name = "proc")]
unsafe extern "C" {
    fn proc_pidpath(pid: libc::c_int, buffer: *mut libc::c_void, size: u32) -> libc::c_int;
}

pub(super) fn executable(pid: u32) -> Option<PathBuf> {
    let mut buffer = vec![0_u8; 4096];
    // SAFETY: `buffer` is writable for its reported size; the call only reads
    // the kernel's record of the process executable.
    let length = unsafe {
        proc_pidpath(
            pid as libc::c_int,
            buffer.as_mut_ptr().cast::<libc::c_void>(),
            buffer.len() as u32,
        )
    };
    if length <= 0 {
        return None;
    }
    let path = CStr::from_bytes_until_nul(&buffer).ok()?;
    Some(PathBuf::from(path.to_string_lossy().as_ref()))
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
    use super::{decode_arguments, kernel_state};

    #[test]
    fn exiting_and_zombie_states_are_distinct_from_running() {
        // SRUN = 2, SSLEEP = 3, SZOMB = 5; P_WEXIT = 0x2000.
        assert_eq!(kernel_state(5, 0x2000), "Z");
        assert_eq!(kernel_state(2, 0x2000), "X");
        assert_eq!(kernel_state(3, 0), "S");
    }

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
