//! The C ABI `include/loopflow.h` declares, in Ghostty's embedding shape.
//! `lf_read` and `lf_subscribe` take a JSON [`Read`]: `lf_read` returns what
//! its `lf` command prints, and `lf_subscribe` serves its `--watch`: Loopflow
//! calls a data-free wakeup from any thread, and the app pulls the frames that
//! command prints on its own queue. `lf_write` takes a JSON [`Write`] and
//! returns what its `lf` command prints. Results are the DTOs Swift already
//! decodes. Every entry point catches panics, so a Rust panic reaches the app
//! as an error, never a crash. The library opens only the store `lf` prepared,
//! and never creates or migrates one.

use std::ffi::{c_char, c_void, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

use loopflow::lf::commands::read::Read;
use loopflow::lf::commands::watch::{panic_message, Subscription};
use loopflow::lf::commands::write::Write;

/// One served watch, owned by the app until `lf_unsubscribe`.
#[derive(Debug)]
pub struct LfSubscription {
    subscription: Subscription,
    wakeup: Arc<Wakeup>,
}

pub type LfWakeup = extern "C" fn(context: *mut c_void);

/// The app's wakeup and its context; `None` once unsubscribed.
#[derive(Debug)]
struct Wakeup(Mutex<Option<(LfWakeup, usize)>>);

impl Wakeup {
    fn call(&self) {
        // Held through the call, so `lf_unsubscribe` cannot return mid-call.
        let target = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        if let Some((wakeup, context)) = *target {
            let _ = catch_unwind(|| wakeup(context as *mut c_void));
        }
    }

    fn end(&self) {
        *self.0.lock().unwrap_or_else(|poison| poison.into_inner()) = None;
    }
}

/// Start serving `read`'s watch; `wakeup` says when to pull.
///
/// # Safety
/// `read` is a NUL-terminated string; `error` is null or writable;
/// `wakeup` accepts calls from any thread with `context`.
#[no_mangle]
pub unsafe extern "C" fn lf_subscribe(
    read: *const c_char,
    wakeup: LfWakeup,
    context: *mut c_void,
    error: *mut *mut c_char,
) -> *mut LfSubscription {
    let target = Arc::new(Wakeup(Mutex::new(Some((wakeup, context as usize)))));
    let wake = target.clone();
    // SAFETY: the caller's guarantees, passed on.
    let subscription = unsafe {
        serve("lf_subscribe", read, error, |read: Read| {
            read.subscribe(move || wake.call())
        })
    };
    subscription.map_or(std::ptr::null_mut(), |subscription| {
        Box::into_raw(Box::new(LfSubscription {
            subscription,
            wakeup: target,
        }))
    })
}

/// Read `read` on this thread: what its `lf` command prints, or null with
/// `*error` set. Free either with `lf_string_free`.
///
/// # Safety
/// `read` is a NUL-terminated string; `error` is null or writable.
#[no_mangle]
pub unsafe extern "C" fn lf_read(read: *const c_char, error: *mut *mut c_char) -> *mut c_char {
    // SAFETY: the caller's guarantees, passed on.
    unsafe { serve("lf_read", read, error, |read: Read| read.read()) }
        .map_or(std::ptr::null_mut(), |json| c_string(&json).into_raw())
}

/// Make `write` on this thread: what its `lf` command prints, or null with
/// `*error` set. Null with `*error` left null when `lf` would send the write
/// to another Machine: nothing was written, and its `lf` command sends it.
/// Free either string with `lf_string_free`.
///
/// # Safety
/// `write` is a NUL-terminated string; `error` is null or writable.
#[no_mangle]
pub unsafe extern "C" fn lf_write(write: *const c_char, error: *mut *mut c_char) -> *mut c_char {
    // SAFETY: the caller's guarantees, passed on.
    unsafe { serve("lf_write", write, error, |write: Write| write.write()) }
        .flatten()
        .map_or(std::ptr::null_mut(), |text| c_string(&text).into_raw())
}

/// `serve` applied to the JSON `request` (a `Read` or a `Write`), or `None`
/// with `*error` set when it is invalid, fails or panics.
///
/// # Safety
/// `request` is a NUL-terminated string; `error` is null or writable.
unsafe fn serve<R: serde::de::DeserializeOwned, T, E: std::fmt::Display>(
    entry: &str,
    request: *const c_char,
    error: *mut *mut c_char,
    serve: impl FnOnce(R) -> Result<T, E>,
) -> Option<T> {
    let served = catch_unwind(AssertUnwindSafe(|| {
        loopflow::store::serve_in_app();
        // SAFETY: the caller passes a NUL-terminated string.
        let request = unsafe { CStr::from_ptr(request) };
        let request = serde_json::from_slice(request.to_bytes())
            .map_err(|error| format!("invalid request: {error}"))?;
        serve(request).map_err(|error| format!("{error:#}"))
    }));
    let failure = match served {
        Ok(Ok(value)) => return Some(value),
        Ok(Err(failure)) => failure,
        Err(panic) => format!("{entry} panicked: {}", panic_message(&panic)),
    };
    // SAFETY: the caller passes a writable pointer or null.
    unsafe { set_error(error, &failure) };
    None
}

fn c_string(text: &str) -> CString {
    CString::new(text.replace('\0', " ")).expect("NUL bytes were replaced")
}

/// # Safety
/// `error` is null or writable.
unsafe fn set_error(error: *mut *mut c_char, message: &str) {
    if !error.is_null() {
        // SAFETY: checked non-null; the caller guarantees it is writable.
        unsafe { *error = c_string(message).into_raw() };
    }
}

/// Deliver one request line to the watch.
///
/// # Safety
/// `subscription` came from `lf_subscribe` and is not yet unsubscribed;
/// `line` points to `length` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn lf_request(
    subscription: *mut LfSubscription,
    line: *const u8,
    length: usize,
) {
    if subscription.is_null() || line.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: the caller passes a live subscription and `length` bytes.
        let (subscription, line) =
            unsafe { (&*subscription, std::slice::from_raw_parts(line, length)) };
        subscription.subscription.request(line);
    }));
}

/// The newest changed frame, or null.
///
/// # Safety
/// `subscription` came from `lf_subscribe` and is not yet unsubscribed;
/// `length` is writable.
#[no_mangle]
pub unsafe extern "C" fn lf_pull(subscription: *mut LfSubscription, length: *mut usize) -> *mut u8 {
    if subscription.is_null() || length.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: the caller passes a live subscription.
    let subscription = unsafe { &*subscription };
    // `Subscription::pull` turns a panicking read into an error frame; this
    // catches anything around it.
    let frame = catch_unwind(AssertUnwindSafe(|| subscription.subscription.pull()))
        .ok()
        .flatten();
    let Some(frame) = frame else {
        return std::ptr::null_mut();
    };
    let frame = frame.into_boxed_slice();
    // SAFETY: the caller passes a writable length.
    unsafe { *length = frame.len() };
    Box::into_raw(frame).cast()
}

/// Whether the watch failed and its last frame was pulled; nothing follows.
///
/// # Safety
/// `subscription` came from `lf_subscribe` and is not yet unsubscribed.
#[no_mangle]
pub unsafe extern "C" fn lf_ended(subscription: *mut LfSubscription) -> bool {
    // SAFETY: the caller passes a live subscription or null.
    unsafe { subscription.as_ref() }.is_none_or(|subscription| subscription.subscription.ended())
}

/// Free a frame `lf_pull` returned.
///
/// # Safety
/// `frame` and `length` came from one `lf_pull`; the frame is freed once.
#[no_mangle]
pub unsafe extern "C" fn lf_frame_free(frame: *mut u8, length: usize) {
    if !frame.is_null() {
        // SAFETY: `lf_pull` leaked a boxed slice of exactly `length` bytes.
        drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(frame, length)) });
    }
}

/// End the watch and free it.
///
/// # Safety
/// `subscription` came from `lf_subscribe` and is unsubscribed once.
#[no_mangle]
pub unsafe extern "C" fn lf_unsubscribe(subscription: *mut LfSubscription) {
    if subscription.is_null() {
        return;
    }
    // SAFETY: the caller hands back ownership exactly once.
    let subscription = unsafe { Box::from_raw(subscription) };
    subscription.wakeup.end();
    let _ = catch_unwind(AssertUnwindSafe(move || drop(subscription)));
}

/// Free a string Loopflow returned.
///
/// # Safety
/// `string` came from Loopflow and is freed once.
#[no_mangle]
pub unsafe extern "C" fn lf_string_free(string: *mut c_char) {
    if !string.is_null() {
        // SAFETY: the string was produced by `CString::into_raw`.
        drop(unsafe { CString::from_raw(string) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    extern "C" fn forward(context: *mut c_void) {
        // SAFETY: the test passes a live sender and keeps it until unsubscribed.
        let sender = unsafe { &*(context as *const mpsc::Sender<()>) };
        let _ = sender.send(());
    }

    #[test]
    fn an_unknown_read_is_an_error_not_a_subscription() {
        let read = CString::new(r#"{"read":"rm_rf"}"#).unwrap();
        let mut error = std::ptr::null_mut();
        let subscription =
            unsafe { lf_subscribe(read.as_ptr(), forward, std::ptr::null_mut(), &mut error) };
        assert!(subscription.is_null());
        let message = unsafe { CStr::from_ptr(error) }
            .to_str()
            .unwrap()
            .to_owned();
        unsafe { lf_string_free(error) };
        assert!(message.contains("invalid request"), "{message}");
    }

    #[test]
    fn a_failed_read_is_an_error_not_a_result() {
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        let read = CString::new(r#"{"read":"task_file","issue":"NONE-1","path":"a"}"#).unwrap();
        let mut error = std::ptr::null_mut();
        assert!(unsafe { lf_read(read.as_ptr(), &mut error) }.is_null());
        assert!(!error.is_null());
        unsafe { lf_string_free(error) };
    }

    #[test]
    fn a_wakeup_is_followed_by_a_frame_to_pull() {
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        let (sender, wakes) = mpsc::channel::<()>();
        let context = Box::into_raw(Box::new(sender));
        let read = CString::new(
            r#"{"read":"roadmap","directory":"/","wave":null,"task":null,"all":true}"#,
        )
        .unwrap();
        let mut error = std::ptr::null_mut();
        let subscription =
            unsafe { lf_subscribe(read.as_ptr(), forward, context.cast(), &mut error) };
        assert!(!subscription.is_null());
        wakes.recv_timeout(Duration::from_secs(30)).unwrap();
        let mut length = 0;
        let frame = unsafe { lf_pull(subscription, &mut length) };
        assert!(!frame.is_null());
        let value: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(frame, length) }).unwrap();
        unsafe { lf_frame_free(frame, length) };
        assert!(value["unavailable"].is_null(), "{value}");
        // Nothing changed since: the next pull has nothing.
        assert!(unsafe { lf_pull(subscription, &mut length) }.is_null());
        unsafe { lf_unsubscribe(subscription) };
        // SAFETY: no wakeup follows lf_unsubscribe.
        drop(unsafe { Box::from_raw(context) });
    }
}
