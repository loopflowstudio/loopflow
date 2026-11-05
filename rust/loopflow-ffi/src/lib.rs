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

/// Read the settings catalogue or edit a declaration through the common file owner.
///
/// # Safety
/// `request` is NUL-terminated; `error` is null or writable. Free returned strings.
#[no_mangle]
pub unsafe extern "C" fn lf_config(request: *const c_char, error: *mut *mut c_char) -> *mut c_char {
    // SAFETY: the caller's guarantees are passed through to the shared decoder.
    unsafe {
        serve(
            "lf_config",
            request,
            error,
            |request: loopflow::config::api::Request| {
                loopflow::config::api::run(request)
                    .and_then(|value| Ok(serde_json::to_string(&value)?))
            },
        )
    }
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
    use super::{lf_frame_free, lf_pull, lf_read, lf_string_free, lf_subscribe, lf_unsubscribe};
    use std::ffi::{c_void, CStr, CString};
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
        let _env = loopflow_test_support::env_lock().lock().unwrap();
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
        let _env = loopflow_test_support::env_lock().lock().unwrap();
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

#[cfg(test)]
mod configuration_tests {
    use super::{lf_config, lf_string_free};
    use loopflow::config::api::Request;
    use loopflow::config::files::Destination;
    use loopflow_test_support::TestRepo;
    use serde_json::{json, Value};
    use std::ffi::{CStr, CString};
    use std::path::Path;

    fn request(request: Request) -> Result<Value, String> {
        let request = CString::new(serde_json::to_string(&request).unwrap()).unwrap();
        let mut error = std::ptr::null_mut();
        // SAFETY: live NUL-terminated request and writable error; both returned strings are freed.
        unsafe {
            let result = lf_config(request.as_ptr(), &mut error);
            if !error.is_null() {
                let message = CStr::from_ptr(error).to_string_lossy().into_owned();
                lf_string_free(error);
                return Err(message);
            }
            assert!(!result.is_null());
            let value = serde_json::from_slice(CStr::from_ptr(result).to_bytes()).unwrap();
            lf_string_free(result);
            Ok(value)
        }
    }

    fn edit(
        destination: &Destination,
        key: &str,
        value: Option<&str>,
        unset: bool,
    ) -> Result<Value, String> {
        request(Request::Declaration {
            destination: destination.clone(),
            key: key.into(),
            value: value.map(str::to_owned),
            unset,
        })
    }

    fn check(
        destination: &Destination,
        repo: &Path,
        wave: Option<&str>,
        key: &str,
        expected: &str,
    ) {
        let explained = request(Request::Explain {
            destination: destination.clone(),
            key: key.into(),
        })
        .unwrap();
        let config = loopflow::config::load_wave_config(Some(repo), wave)
            .unwrap()
            .unwrap();
        let skill = if key.starts_with("graphs.") {
            let graph = loopflow::graph::load_authored_graph("release", repo).unwrap();
            let steps = loopflow::graph::compile_graph(&graph, repo).unwrap();
            let loopflow::graph::ConcreteStep::Skill(step) = &steps[0] else {
                panic!("expected skill")
            };
            step.skill.clone()
        } else {
            loopflow::graph::load_skill("probe", repo).unwrap()
        };
        let prepared = loopflow::prompt::process::prepare_process_prompt(
            &config,
            loopflow::prompt::process::ProcessPromptInput {
                repo_root: repo.to_owned(),
                wave: wave.map(str::to_owned),
                resolved_skill: Some(skill),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(prepared.config.agent().unwrap(), expected);
        assert_eq!(
            explained["inspections"][0]["settings"]["agent"]["value"],
            expected
        );
    }

    #[test]
    fn destinations_bind_ffi_save_explanation_and_launch_across_worktrees_and_wave_renames() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let previous = std::env::var_os("LF_HOME");
        std::env::set_var("LF_HOME", home.path());
        let repo = TestRepo::new();
        repo.create_file(".lf/skills/probe.md", "Probe settings.\n");
        repo.create_file(
            ".lf/graphs/release.yaml",
            "- step: {name: probe, id: work}\n",
        );
        let id = "8ae21a68-8940-4553-a3ab-135133925e35";
        repo.create_file(".lf/wave/old/config.yaml", &format!("id: {id}\n"));
        repo.stage_all();
        repo.commit("Authored fixture");
        let child = home.path().join("child");
        assert!(std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(["worktree", "add", "-b", "child"])
            .arg(&child)
            .output()
            .unwrap()
            .status
            .success());
        let global = Destination::PersonalGlobal {};
        let private = Destination::PersonalRepository {
            repository: child.clone(),
        };
        let shared = Destination::SharedRepository {
            repository: child.clone(),
        };
        let wave = Destination::PersonalWave {
            repository: child.clone(),
            wave: "old".into(),
        };
        let other = Destination::PersonalWave {
            repository: child.clone(),
            wave: "different".into(),
        };
        std::fs::create_dir_all(child.join(".lf/wave/different")).unwrap();
        std::fs::write(
            child.join(".lf/wave/different/config.yaml"),
            "id: 9ae21a68-8940-4553-a3ab-135133925e35\n",
        )
        .unwrap();
        edit(&global, "default_agent", Some("codex:global"), false).unwrap();
        let global_before = std::fs::read(home.path().join("config.yaml")).unwrap();
        edit(&shared, "agent", Some("claude:shared"), false).unwrap();
        let saved = edit(&private, "skills.probe.agent", Some("codex:private"), false).unwrap();
        assert_eq!(
            saved["path"],
            json!(repo
                .path()
                .canonicalize()
                .unwrap()
                .join("lf.config.local.yaml"))
        );
        assert!(!child.join("lf.config.local.yaml").exists());
        check(
            &private,
            &child,
            None,
            "skills.probe.agent",
            "codex:private",
        );
        check(
            &private,
            repo.path(),
            None,
            "skills.probe.agent",
            "codex:private",
        );
        edit(&other, "skills.probe.agent", Some("claude:other"), false).unwrap();
        edit(&wave, "skills.probe.agent", Some("claude:wave"), false).unwrap();
        check(
            &wave,
            &child,
            Some("old"),
            "skills.probe.agent",
            "claude:wave",
        );
        let private_before = std::fs::read(repo.path().join("lf.config.local.yaml")).unwrap();
        assert!(edit(&private, "user.name", Some("Wrong scope"), false)
            .unwrap_err()
            .contains("does not apply"));
        assert!(edit(&shared, "user.name", Some("Wrong owner"), false).is_err());
        assert_eq!(
            std::fs::read(repo.path().join("lf.config.local.yaml")).unwrap(),
            private_before
        );
        std::fs::rename(child.join(".lf/wave/old"), child.join(".lf/wave/new")).unwrap();
        std::fs::create_dir_all(child.join(".lf/wave/old")).unwrap();
        std::fs::write(
            child.join(".lf/wave/old/config.yaml"),
            "id: 7ae21a68-8940-4553-a3ab-135133925e35\n",
        )
        .unwrap();
        let renamed = Destination::PersonalWave {
            repository: child.clone(),
            wave: id.into(),
        };
        assert_eq!(
            edit(&renamed, "skills.probe.agent", None, false).unwrap()["value"],
            "claude:wave"
        );
        check(
            &renamed,
            &child,
            Some("new"),
            "skills.probe.agent",
            "claude:wave",
        );
        check(
            &wave,
            &child,
            Some("old"),
            "skills.probe.agent",
            "codex:private",
        );
        edit(&renamed, "skills.probe.agent", None, true).unwrap();
        check(
            &renamed,
            &child,
            Some("new"),
            "skills.probe.agent",
            "codex:private",
        );
        check(
            &other,
            &child,
            Some("different"),
            "skills.probe.agent",
            "claude:other",
        );
        let shared_wave = Destination::SharedWave {
            repository: child.clone(),
            wave: id.into(),
        };
        let saved = edit(&shared_wave, "chrome", Some("false"), false).unwrap();
        assert_eq!(
            saved["path"],
            json!(child
                .canonicalize()
                .unwrap()
                .join(".lf/wave/new/config.yaml"))
        );
        edit(
            &private,
            "graphs.release.steps.work.agent",
            Some("claude:occurrence"),
            false,
        )
        .unwrap();
        check(
            &private,
            &child,
            None,
            "graphs.release.steps.work.agent",
            "claude:occurrence",
        );
        edit(&private, "graphs.release.steps.work.agent", None, true).unwrap();
        edit(&private, "skills.probe.agent", None, true).unwrap();
        check(
            &private,
            &child,
            None,
            "skills.probe.agent",
            "claude:shared",
        );
        edit(&shared, "agent", None, true).unwrap();
        check(
            &private,
            &child,
            None,
            "skills.probe.default_agent",
            "codex:global",
        );
        assert_eq!(
            std::fs::read(home.path().join("config.yaml")).unwrap(),
            global_before
        );
        assert!(!home.path().join("loopflow.db").exists());
        let clone = TestRepo::new();
        assert_eq!(
            loopflow::config::load_config(Some(clone.path()))
                .unwrap()
                .unwrap()
                .agent()
                .unwrap(),
            "codex:global"
        );
        match previous {
            Some(value) => std::env::set_var("LF_HOME", value),
            None => std::env::remove_var("LF_HOME"),
        }
    }
}
