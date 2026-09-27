//! Transient filesystem invalidation. Notifications never establish ownership.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const MAX_PATHS: usize = 4096;
const MAX_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Default)]
pub(super) struct Changes {
    pub paths: BTreeSet<PathBuf>,
    pub rescan: bool,
    bytes: usize,
}

impl Changes {
    pub(super) fn insert(&mut self, path: PathBuf) {
        if self.rescan || self.paths.contains(&path) {
            return;
        }
        self.bytes += path.as_os_str().len();
        if self.paths.len() >= MAX_PATHS || self.bytes > MAX_BYTES {
            self.invalidate();
        } else {
            self.paths.insert(path);
        }
    }

    pub(super) fn invalidate(&mut self) {
        self.paths.clear();
        self.bytes = 0;
        self.rescan = true;
    }
}

#[cfg(test)]
mod tests {
    use super::Changes;
    use std::path::PathBuf;

    #[test]
    fn overflow_invalidates_coverage_instead_of_evicting_paths() {
        let mut changes = Changes::default();
        for index in 0..5000 {
            changes.insert(PathBuf::from(format!("runs/{index}")));
        }
        assert!(changes.rescan);
        assert!(changes.paths.is_empty());
        assert_eq!(changes.bytes, 0);
    }
}

// Keep directory events: an older Run can acquire its first native client.
#[cfg(target_os = "macos")]
fn relevant(home: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(home) else {
        return home.starts_with(path);
    };
    let parts: Vec<_> = relative.iter().collect();
    let published = || {
        path.extension().is_some_and(|ext| ext == "json")
            && path
                .file_name()
                .is_some_and(|name| !name.as_encoded_bytes().starts_with(b"."))
    };
    match parts.first().and_then(|value| value.to_str()) {
        None => true,
        Some("runs") => {
            parts.len() <= 3
                || (parts.len() == 4
                    && (parts[3] == "provider-clients" || parts[3] == "manifest.json"))
                || (parts.len() == 5 && parts[3] == "provider-clients" && published())
        }
        Some("run-bindings") => parts.len() == 1 || (parts.len() == 2 && published()),
        Some("runtime") => {
            parts.len() == 1
                || (parts.len() == 2
                    && (parts[1] == "exec-processes" || parts[1] == "opencode-servers.json"))
                || (parts.len() == 3 && parts[1] == "exec-processes" && published())
        }
        _ => false,
    }
}

#[cfg(target_os = "macos")]
pub(super) use macos::Subscription;

#[cfg(not(target_os = "macos"))]
#[derive(Debug)]
pub(super) struct Subscription;

#[cfg(not(target_os = "macos"))]
impl Subscription {
    pub fn start(_: &Path) -> anyhow::Result<Self> {
        anyhow::bail!("continuous active Run discovery requires macOS FSEvents")
    }

    pub fn changes(&self) -> Changes {
        unreachable!("unsupported platforms cannot start a subscription")
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::{c_char, c_void, CStr, CString};
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::ptr;
    use std::sync::Mutex;

    use anyhow::{bail, Result};

    use super::{relevant, Changes};

    type Ref = *mut c_void;
    type Callback = unsafe extern "C" fn(Ref, Ref, usize, Ref, *const u32, *const u64);

    #[repr(C)]
    struct Context {
        version: isize,
        info: Ref,
        retain: Option<unsafe extern "C" fn(Ref) -> Ref>,
        release: Option<unsafe extern "C" fn(Ref)>,
        description: Option<unsafe extern "C" fn(Ref) -> Ref>,
    }

    #[link(name = "CoreServices", kind = "framework")]
    unsafe extern "C" {
        fn FSEventStreamCreate(
            allocator: Ref,
            callback: Callback,
            context: *mut Context,
            paths: Ref,
            since: u64,
            latency: f64,
            flags: u32,
        ) -> Ref;
        fn FSEventStreamSetDispatchQueue(stream: Ref, queue: Ref);
        fn FSEventStreamStart(stream: Ref) -> u8;
        fn FSEventStreamFlushSync(stream: Ref);
        fn FSEventStreamStop(stream: Ref);
        fn FSEventStreamInvalidate(stream: Ref);
        fn FSEventStreamRelease(stream: Ref);
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringCreateWithFileSystemRepresentation(allocator: Ref, path: *const c_char) -> Ref;
        fn CFArrayCreate(allocator: Ref, values: *const Ref, count: isize, callbacks: Ref) -> Ref;
        fn CFRelease(value: Ref);
    }

    unsafe extern "C" {
        fn dispatch_queue_create(label: *const c_char, attribute: Ref) -> Ref;
        fn dispatch_sync_f(queue: Ref, context: Ref, function: unsafe extern "C" fn(Ref));
        fn dispatch_release(object: Ref);
    }

    #[derive(Debug)]
    struct State {
        home: PathBuf,
        changes: Mutex<Changes>,
    }

    #[derive(Debug)]
    pub(crate) struct Subscription {
        stream: Ref,
        queue: Ref,
        paths: Ref,
        root: Ref,
        state: Box<State>,
    }

    // SAFETY: the handle has one owner; callbacks touch only the boxed Mutex.
    // FSEvents/dispatch objects are not thread-affine. No concurrent stream calls.
    unsafe impl Send for Subscription {}

    unsafe extern "C" fn callback(
        _: Ref,
        info: Ref,
        count: usize,
        paths: Ref,
        flags: *const u32,
        _: *const u64,
    ) {
        // SAFETY: FSEvents supplies count-sized arrays and our live boxed context.
        // Subscription drains the callback queue before releasing the box.
        unsafe {
            let state = &*info.cast::<State>();
            let mut changes = state
                .changes
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            for index in 0..count {
                let flags = *flags.add(index);
                // MustScanSubDirs, UserDropped, KernelDropped, EventIdsWrapped,
                // RootChanged, Mount, Unmount. Interpret these before filtering.
                if flags & 0xef != 0 {
                    changes.invalidate();
                    continue;
                }
                let bytes = CStr::from_ptr(*paths.cast::<*const c_char>().add(index)).to_bytes();
                let path = Path::new(std::ffi::OsStr::from_bytes(bytes));
                if relevant(&state.home, path) {
                    changes.insert(path.to_owned());
                }
            }
        }
    }

    unsafe extern "C" fn barrier(_: Ref) {}

    impl Subscription {
        pub fn start(home: &Path) -> Result<Self> {
            let root = home
                .ancestors()
                .find(|path| path.is_dir())
                .ok_or_else(|| anyhow::anyhow!("no existing Home ancestor to observe"))?;
            let root = CString::new(root.as_os_str().as_bytes())?;
            let mut state = Box::new(State {
                home: home.to_owned(),
                changes: Mutex::new(Changes::default()),
            });
            // SAFETY: all CF objects remain alive for the stream lifetime. The
            // context is boxed and stable; the callback executes on its own queue.
            unsafe {
                let root =
                    CFStringCreateWithFileSystemRepresentation(ptr::null_mut(), root.as_ptr());
                if root.is_null() {
                    bail!("cannot encode filesystem event root");
                }
                let paths = CFArrayCreate(ptr::null_mut(), &root, 1, ptr::null_mut());
                if paths.is_null() {
                    CFRelease(root);
                    bail!("cannot allocate event roots");
                }
                let queue =
                    dispatch_queue_create(c"loopflow.active-runs".as_ptr(), ptr::null_mut());
                let mut context = Context {
                    version: 0,
                    info: (&mut *state as *mut State).cast(),
                    retain: None,
                    release: None,
                    description: None,
                };
                let stream = FSEventStreamCreate(
                    ptr::null_mut(),
                    callback,
                    &mut context,
                    paths,
                    u64::MAX,
                    0.05,
                    0x14, // WatchRoot | FileEvents; events since subscription.
                );
                if stream.is_null() {
                    dispatch_release(queue);
                    CFRelease(paths);
                    CFRelease(root);
                    bail!("cannot create filesystem event stream");
                }
                let subscription = Self {
                    stream,
                    queue,
                    paths,
                    root,
                    state,
                };
                FSEventStreamSetDispatchQueue(stream, queue);
                if FSEventStreamStart(stream) == 0 {
                    bail!("cannot start filesystem event stream");
                }
                Ok(subscription)
            }
        }

        pub fn changes(&self) -> Changes {
            // SAFETY: only the reader calls this, never its callback queue. The
            // SDK guarantees delivery of preceding events when FlushSync returns.
            unsafe {
                FSEventStreamFlushSync(self.stream);
            }
            std::mem::take(
                &mut *self
                    .state
                    .changes
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()),
            )
        }
    }

    impl Drop for Subscription {
        fn drop(&mut self) {
            // SAFETY: stop scheduling callbacks, drain any already submitted,
            // then release the stream/queue/CF values before the context box.
            unsafe {
                FSEventStreamStop(self.stream);
                FSEventStreamInvalidate(self.stream);
                dispatch_sync_f(self.queue, ptr::null_mut(), barrier);
                FSEventStreamRelease(self.stream);
                dispatch_release(self.queue);
                CFRelease(self.paths);
                CFRelease(self.root);
            }
        }
    }
}
