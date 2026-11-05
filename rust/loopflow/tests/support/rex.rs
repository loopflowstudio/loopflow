//! One isolated Rex server. It refuses unqualified CLI requests so a fixture can
//! never reach the person's server, and owns every terminal process it starts.
#![allow(dead_code)] // Shared native fixture compiled into multiple test crates.

use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

pub struct IsolatedRex {
    pub endpoint: String,
    rex: PathBuf,
    root: PathBuf,
    bin: PathBuf,
    lf_bin: PathBuf,
    server: Option<Child>,
}

impl IsolatedRex {
    /// `bin` receives the guarded `rex`; native children see it first on PATH.
    pub fn start(root: &Path, bin: &Path, lf_bin: &Path) -> Self {
        let rex: PathBuf = std::env::var_os("LOOPFLOW_TEST_REX")
            .expect("set LOOPFLOW_TEST_REX to the installed Rex executable")
            .into();
        std::fs::write(root.join("empty.lua"), "").unwrap();
        // Native children have no REX_SERVER; the caller must discover this one.
        let guard = bin.join("rex");
        std::fs::write(
            &guard,
            format!(
                "#!/bin/sh\ncase \" $* \" in *' -S '*|*' --server '*) ;; *) echo 'missing explicit Rex endpoint' >&2; exit 91;; esac\nexec '{}' \"$@\"\n",
                rex.to_string_lossy().replace('\'', "'\"'\"'"),
            ),
        )
        .unwrap();
        std::fs::set_permissions(&guard, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut fixture = Self {
            endpoint: format!("unix://{}", root.join("rex.sock").display()),
            rex,
            root: root.into(),
            bin: bin.into(),
            lf_bin: lf_bin.into(),
            server: None,
        };
        fixture.run();
        fixture
    }

    fn run(&mut self) {
        let socket = self.root.join("rex.sock");
        let _ = std::fs::remove_file(&socket);
        let server = Command::new(&self.rex)
            .args(["server", "run", "--listen", &self.endpoint, "--state-dir"])
            .arg(self.root.join("state"))
            .arg("--config")
            .arg(self.root.join("empty.lua"))
            .env_clear()
            .env("HOME", &self.root)
            .env("LF_BIN", &self.lf_bin)
            .env(
                "PATH",
                format!(
                    "{}:{}:/opt/homebrew/bin:/usr/bin:/bin",
                    self.bin.display(),
                    self.rex.parent().unwrap().display()
                ),
            )
            .env("TERM", "xterm-256color")
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        self.server = Some(server);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !socket.exists() {
            let server = self.server.as_mut().unwrap();
            assert!(server.try_wait().unwrap().is_none(), "isolated Rex exited");
            assert!(Instant::now() < deadline, "isolated Rex did not listen");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Stop the server and every terminal in it. Rex saves asynchronously: a
    /// shutdown right after creation can lose the newest session.
    pub fn stop(&mut self) {
        std::thread::sleep(Duration::from_secs(2));
        if let Some(mut server) = self.server.take() {
            // SAFETY: each fixture starts its server as a new process group leader.
            unsafe { libc::kill(-(server.id() as i32), libc::SIGTERM) };
            server.wait().unwrap();
        }
    }

    pub fn restart(&mut self) {
        self.stop();
        self.run();
    }

    fn cli(&self) -> Command {
        let mut command = Command::new(&self.rex);
        for name in ["REX_SERVER", "REX_SESSION", "REX_BLOCK"] {
            command.env_remove(name);
        }
        command.args(["--autostart=false", "-S", &self.endpoint]);
        command
    }

    pub fn call(&self, method: &str, args: Value) -> Value {
        let output = self
            .cli()
            .args(["api", "call", method, &args.to_string()])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{method}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }

    pub fn block_ids(&self, session: &str) -> Vec<String> {
        let mut ids: Vec<String> = self.call(
            "session.list_blocks",
            serde_json::json!({"session_id":session}),
        )["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|block| block["block_id"].as_str().unwrap().to_owned())
            .collect();
        ids.sort();
        ids
    }

    pub fn block_call(&self, session: &str, block: &str, method: &str) -> Value {
        let output = self
            .cli()
            .args(["block", "call", "-s", session, "-b", block, method])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{method}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }

    /// Type into a terminal the way a person would.
    pub fn write(&self, session: &str, block: &str, data: &str) {
        let output = self
            .cli()
            .args(["block", "call", "-s", session, "-b", block, "write"])
            .arg(serde_json::json!({"data":data}).to_string())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    pub fn screen(&self, session: &str, block: &str) -> String {
        let output = self
            .cli()
            .args(["block", "capture", "-s", session, "-b", block])
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).into_owned()
    }
}

impl Drop for IsolatedRex {
    fn drop(&mut self) {
        if let Some(mut server) = self.server.take() {
            // SAFETY: each fixture starts its server as a new process group leader.
            unsafe { libc::kill(-(server.id() as i32), libc::SIGTERM) };
            let _ = server.wait();
        }
    }
}
