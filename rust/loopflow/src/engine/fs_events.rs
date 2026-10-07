//! macOS filesystem events for one or more directory trees. An event only says
//! "look again"; it never establishes what is there.

use std::ffi::{c_char, c_void, CStr, CString};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::ptr;

use anyhow::{bail, Result};

type Ref = *mut c_void;
type Callback = unsafe extern "C" fn(Ref, Ref, usize, Ref, *const u32, *const u64);

/// Called with each changed path, or `None` when events were lost and
/// everything under the roots may have changed.
type Sink = Box<dyn Fn(Option<&Path>) + Send + Sync>;

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

pub(crate) struct Stream {
    stream: Ref,
    queue: Ref,
    paths: Ref,
    roots: Vec<Ref>,
    // Boxed twice: the callback needs a thin, stable pointer.
    _sink: Box<Sink>,
}

impl std::fmt::Debug for Stream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Stream")
            .field("roots", &self.roots.len())
            .finish()
    }
}

// SAFETY: the handle has one owner; callbacks touch only the boxed sink, which
// is Send + Sync. FSEvents/dispatch objects are not thread-affine. No
// concurrent stream calls.
unsafe impl Send for Stream {}

unsafe extern "C" fn callback(
    _: Ref,
    info: Ref,
    count: usize,
    paths: Ref,
    flags: *const u32,
    _: *const u64,
) {
    // SAFETY: FSEvents supplies count-sized arrays and our live boxed sink.
    // Stream drains the callback queue before releasing the box.
    unsafe {
        let sink = &*info.cast::<Sink>();
        for index in 0..count {
            // MustScanSubDirs, UserDropped, KernelDropped, EventIdsWrapped,
            // RootChanged, Mount, Unmount.
            if *flags.add(index) & 0xef != 0 {
                sink(None);
                continue;
            }
            let bytes = CStr::from_ptr(*paths.cast::<*const c_char>().add(index)).to_bytes();
            sink(Some(Path::new(std::ffi::OsStr::from_bytes(bytes))));
        }
    }
}

unsafe extern "C" fn barrier(_: Ref) {}

impl Stream {
    /// Report changes under `roots` from now on, at most every `latency`
    /// seconds. `files` names each changed file; otherwise its directory.
    pub(crate) fn start(
        roots: &[&Path],
        latency: f64,
        files: bool,
        sink: impl Fn(Option<&Path>) + Send + Sync + 'static,
    ) -> Result<Self> {
        let names = roots
            .iter()
            .map(|root| CString::new(root.as_os_str().as_bytes()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut sink: Box<Sink> = Box::new(Box::new(sink));
        // SAFETY: all CF objects remain alive for the stream lifetime. The
        // sink is boxed and stable; the callback executes on its own queue.
        unsafe {
            let mut encoded = Vec::with_capacity(names.len());
            let release = |encoded: &[Ref]| encoded.iter().for_each(|root| CFRelease(*root));
            for name in &names {
                let root =
                    CFStringCreateWithFileSystemRepresentation(ptr::null_mut(), name.as_ptr());
                if root.is_null() {
                    release(&encoded);
                    bail!("cannot encode filesystem event root");
                }
                encoded.push(root);
            }
            let paths = CFArrayCreate(
                ptr::null_mut(),
                encoded.as_ptr(),
                encoded.len() as isize,
                ptr::null_mut(),
            );
            if paths.is_null() {
                release(&encoded);
                bail!("cannot allocate event roots");
            }
            let queue = dispatch_queue_create(c"loopflow.fs-events".as_ptr(), ptr::null_mut());
            let mut context = Context {
                version: 0,
                info: (&mut *sink as *mut Sink).cast(),
                retain: None,
                release: None,
                description: None,
            };
            let stream = FSEventStreamCreate(
                ptr::null_mut(),
                callback,
                &mut context,
                paths,
                u64::MAX, // events since subscription
                latency,
                if files { 0x14 } else { 0x04 }, // WatchRoot, FileEvents
            );
            if stream.is_null() {
                dispatch_release(queue);
                CFRelease(paths);
                release(&encoded);
                bail!("cannot create filesystem event stream");
            }
            let started = Self {
                stream,
                queue,
                paths,
                roots: encoded,
                _sink: sink,
            };
            FSEventStreamSetDispatchQueue(stream, queue);
            if FSEventStreamStart(stream) == 0 {
                bail!("cannot start filesystem event stream");
            }
            Ok(started)
        }
    }

    /// Deliver every event that preceded this call before returning.
    pub(crate) fn flush(&self) {
        // SAFETY: only the owner calls this, never the callback queue. The
        // SDK guarantees delivery of preceding events when FlushSync returns.
        unsafe {
            FSEventStreamFlushSync(self.stream);
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        // SAFETY: stop scheduling callbacks, drain any already submitted,
        // then release the stream/queue/CF values before the sink box.
        unsafe {
            FSEventStreamStop(self.stream);
            FSEventStreamInvalidate(self.stream);
            dispatch_sync_f(self.queue, ptr::null_mut(), barrier);
            FSEventStreamRelease(self.stream);
            dispatch_release(self.queue);
            CFRelease(self.paths);
            self.roots.iter().for_each(|root| CFRelease(*root));
        }
    }
}
