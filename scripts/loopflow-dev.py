#!/usr/bin/env python3
"""Development commands for the Loopflow Swift app.

Usage:
    uv run python scripts/loopflow-dev.py <command>

Commands:
    setup           Install/check repo dev environment tools
    build           Build the app
    test            Build and run tests
    run             Build and launch the app
    run-debug       Build and run with stdout visible
    run-debug --with-lfd
                    Also run one-shot local lfd from this branch (for daemon debugging)
    release         Build release .app and .dmg (delegates to release-loopflow.py)
    clean           Remove dev app and reset permissions
    xcode           Open in Xcode
    logs            Tail the app logs
    lfd             Stop installed lfd and run from this branch (native/sqlite)
    lfd -k          Aggressive preflight kill before starting lfd
    agent-image     Build the Docker agent image

    screenshots     Generate app screenshots

Streaming logs (long-running commands):
    ~/.lf/logs/dev/<repo>.lfd.log
    ~/.lf/logs/dev/<repo>.loopflow-run-debug.log
"""

import argparse
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import TextIO

REPO_ROOT = Path(__file__).parent.parent
SWIFT_DIR = REPO_ROOT / "swift"
DEV_APP = Path.home() / "Applications" / "Loopflow Dev.app"
# Dev build runs under its own bundle id so it keeps preferences separate from
# the installed Loopflow app.
DEV_BUNDLE_ID = "com.loopflow.mac.dev"
# Stable self-signed identity used to sign dev builds. Ad-hoc signing (`--sign -`)
# yields a fresh cdhash every build, so the keychain ACL that "Always Allow"
# stays stable across builds and macOS keeps previously granted permissions.
DEV_SIGNING_IDENTITY = "Loopflow Dev"
LOGIN_KEYCHAIN = Path.home() / "Library" / "Keychains" / "login.keychain-db"
ENV_SETUP = REPO_ROOT / ".lf" / "env-setup.sh"
DEV_LOG_DIR = Path.home() / ".lf" / "logs" / "dev"
LFD_STREAM_LOG = DEV_LOG_DIR / f"{REPO_ROOT.name}.lfd.log"
LOOPFLOW_STREAM_LOG = DEV_LOG_DIR / f"{REPO_ROOT.name}.loopflow-run-debug.log"


def run(cmd: list[str], cwd: Path | None = None, check: bool = True) -> subprocess.CompletedProcess:
    """Run a command, optionally checking return code."""
    return subprocess.run(cmd, cwd=cwd, check=check)


def run_capture(cmd: list[str], cwd: Path | None = None) -> subprocess.CompletedProcess:
    """Run a command and capture output."""
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)


def stream_with_log(
    cmd: list[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    log_path: Path,
) -> int:
    """Run a long-lived command, streaming stdout and teeing to a known log file."""
    DEV_LOG_DIR.mkdir(parents=True, exist_ok=True)
    with log_path.open("a", encoding="utf-8") as log_file:
        log_file.write(f"\n\n=== {time.strftime('%Y-%m-%d %H:%M:%S')} ===\n")
        log_file.write(f"$ {' '.join(cmd)}\n\n")
        log_file.flush()

        process = subprocess.Popen(
            cmd,
            cwd=cwd,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )

        try:
            if process.stdout is None:
                return process.wait()
            for line in process.stdout:
                print(line, end="")
                log_file.write(line)
                log_file.flush()
        except KeyboardInterrupt:
            print("\nStopping process...")
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
        return process.wait()


def run_app_bundle_with_log(
    app_path: Path,
    log_path: Path,
    args: list[str] | None = None,
    env: dict[str, str] | None = None,
) -> int:
    """Launch app bundle through LaunchServices and stream redirected stdout/stderr.

    `env` entries are passed to the launched app via `open --env` (LaunchServices
    does not inherit this process's environment).
    """
    DEV_LOG_DIR.mkdir(parents=True, exist_ok=True)

    open_cmd = [
        "open",
        "-n",
        "-W",
        "--stdout",
        str(log_path),
        "--stderr",
        str(log_path),
    ]
    for key, value in (env or {}).items():
        open_cmd.extend(["--env", f"{key}={value}"])
    open_cmd.append(str(app_path))
    if args:
        open_cmd.extend(["--args", *args])
    with log_path.open("a", encoding="utf-8") as log_file:
        log_file.write(f"\n\n=== {time.strftime('%Y-%m-%d %H:%M:%S')} ===\n")
        log_file.write(f"$ {' '.join(open_cmd)}\n\n")
        log_file.flush()

    tail_process = subprocess.Popen(["tail", "-n", "0", "-F", str(log_path)])
    try:
        return run(open_cmd, check=False).returncode
    except KeyboardInterrupt:
        print("\nStopping app...")
        _stop_loopflow_app(app_path)
        return 130
    finally:
        if tail_process.poll() is None:
            tail_process.terminate()
            try:
                tail_process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                tail_process.kill()


def _stop_loopflow_app(app_path: Path) -> None:
    """Quit Loopflow Dev, falling back to killing the app if a modal blocks quit."""
    executable = app_path / "Contents" / "MacOS" / "Loopflow"

    run(["osascript", "-e", 'tell application id "com.loopflow.mac" to quit'], check=False)
    if _wait_for_process_exit(str(executable), timeout_seconds=3):
        return

    print("App did not quit cleanly; forcing shutdown...")
    run(["pkill", "-f", str(executable)], check=False)
    _wait_for_process_exit(str(executable), timeout_seconds=2)


def _wait_for_process_exit(pattern: str, timeout_seconds: float) -> bool:
    deadline = time.time() + timeout_seconds
    while time.time() < deadline:
        result = run_capture(["pgrep", "-f", pattern])
        if result.returncode != 0:
            return True
        time.sleep(0.1)
    return False


def _read_session_token() -> str | None:
    token_path = Path.home() / ".lf" / "session-token"
    if not token_path.exists():
        return None
    token = token_path.read_text(encoding="utf-8").strip()
    return token or None


def _status_ok(token: str | None) -> bool:
    request = urllib.request.Request("http://127.0.0.1:2486/status")
    if token:
        request.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(request, timeout=1.5) as response:
            return response.status == 200
    except (urllib.error.URLError, TimeoutError):
        return False


def _wait_for_lfd_ready(timeout_seconds: float = 20.0) -> bool:
    deadline = time.time() + timeout_seconds
    while time.time() < deadline:
        if _status_ok(_read_session_token()):
            return True
        time.sleep(0.25)
    return False


def _resolve_lfd_sqlite_path(db_path: str) -> Path:
    candidate = Path(db_path)
    if candidate.is_absolute():
        return candidate
    return Path.home() / ".lf" / candidate


def _remove_sqlite_database(db_path: Path) -> None:
    removed_any = False
    for suffix in ("", "-wal", "-shm"):
        candidate = Path(f"{db_path}{suffix}")
        if candidate.exists():
            candidate.unlink()
            removed_any = True
    if removed_any:
        print(f"Reset sqlite DB: {db_path}")


def _reset_run_debug_databases(with_lfd: bool) -> None:
    if with_lfd:
        db_path = os.environ.get("LFD_DB_PATH", f"lfd-{REPO_ROOT.name}.db")
        _remove_sqlite_database(_resolve_lfd_sqlite_path(db_path))


def _start_lfd_background(docker: bool = False) -> tuple[subprocess.Popen[str], TextIO]:
    if docker:
        raise RuntimeError("run-debug --with-lfd currently supports native lfd only")

    env = os.environ.copy()
    env["GRPC_ENABLE_FORK_SUPPORT"] = "0"
    env["GRPC_VERBOSITY"] = "ERROR"
    env.setdefault("LFD_DB_PATH", f"lfd-{REPO_ROOT.name}.db")
    env["RUST_LOG"] = "loopflow=debug,tower_http=debug"

    print("Building lfd...")
    result = run(["cargo", "build", "--locked", "--bin", "lfd"], cwd=REPO_ROOT, check=False)
    if result.returncode != 0:
        raise RuntimeError("Failed to build lfd")

    lfd_bin = str(REPO_ROOT / "target" / "debug" / "lfd")
    DEV_LOG_DIR.mkdir(parents=True, exist_ok=True)
    log_file = LFD_STREAM_LOG.open("a", encoding="utf-8")
    log_file.write(f"\n\n=== {time.strftime('%Y-%m-%d %H:%M:%S')} ===\n")
    log_file.write(f"$ {lfd_bin} serve\n\n")
    log_file.flush()

    print(f"Using sqlite DB: ~/.lf/{env['LFD_DB_PATH']}")
    print(f"Stream log: {LFD_STREAM_LOG}")
    print("Starting lfd from this branch (debug logging enabled)...")
    process = subprocess.Popen(
        [lfd_bin, "serve"],
        cwd=REPO_ROOT,
        env=env,
        stdout=log_file,
        stderr=subprocess.STDOUT,
        text=True,
    )
    return process, log_file


def _stop_process(process: subprocess.Popen[str], name: str) -> None:
    if process.poll() is not None:
        return
    print(f"Stopping {name}...")
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        print(f"Force-killing {name}...")
        process.kill()


# --- Swift commands ---


def cmd_setup(install: bool = False, dry_run: bool = False) -> int:
    """Install or check idempotent repo dev environment tooling."""
    if not ENV_SETUP.exists():
        print(f"Error: missing {ENV_SETUP}")
        return 1

    mode = "--install" if install else "--check"
    cmd = [str(ENV_SETUP), mode]
    if dry_run:
        cmd.append("--dry-run")
    return run(cmd, cwd=REPO_ROOT, check=False).returncode


def cmd_build() -> int:
    """Build the app."""
    print("Building Loopflow...")
    return run(["swift", "build"], cwd=SWIFT_DIR, check=False).returncode


def cmd_test() -> int:
    """Build and run tests."""
    print("Building and testing...")
    result = run(["swift", "build"], cwd=SWIFT_DIR, check=False)
    if result.returncode != 0:
        return result.returncode
    return run(["swift", "test"], cwd=SWIFT_DIR, check=False).returncode


def cmd_run() -> int:
    """Build and launch the app."""
    print("Building and running Loopflow...")
    result = run(["swift", "build"], cwd=SWIFT_DIR, check=False)
    if result.returncode != 0:
        return result.returncode

    _install_dev_app()
    # Dev launches read this checkout's wave/ dir as-is; a plain production
    # launch leaves the override unset and reads the main worktree.
    run(
        [
            "open",
            "-n",
            "--env",
            f"LOOPFLOW_DEV_WAVE_REPO={REPO_ROOT}",
            str(DEV_APP),
            "--args",
            "--repo",
            str(REPO_ROOT),
        ]
    )
    return 0


def _print_run_debug_checklist() -> None:
    """Print the manual review path for Wave Chat and work supervision."""
    print("Review checklist:")
    print("  1. Select a Wave: its conversation and work map should agree on identity.")
    print("  2. Send while idle: Wave Chat should launch or reconnect to `lf serve`.")
    print("  3. Send while turning: the composer should expose Steer and Interrupt & Send.")
    print("  4. Verify Projects, Tasks, decisions, and PR delivery refresh from `lf status`.")
    print("  5. Switch Waves: each conversation should retain its own endpoint and playhead.")


def cmd_run_debug(with_lfd: bool = False, repo: Path = REPO_ROOT) -> int:
    """Build and run with stdout visible. `repo` is the repo the app opens."""
    lfd_process: subprocess.Popen[str] | None = None
    lfd_log: TextIO | None = None

    if with_lfd:
        _stop_installed_lfd()
    _reset_run_debug_databases(with_lfd=with_lfd)

    if with_lfd:
        try:
            lfd_process, lfd_log = _start_lfd_background(docker=False)
        except RuntimeError as error:
            print(str(error))
            return 1

        if not _wait_for_lfd_ready():
            print("lfd did not become ready in time. Check:")
            print(f"  {LFD_STREAM_LOG}")
            if lfd_process is not None:
                _stop_process(lfd_process, "lfd")
            if lfd_log is not None:
                lfd_log.close()
            return 1

    print("Building and running Loopflow (debug mode with logs)...")
    result = run(["swift", "build"], cwd=SWIFT_DIR, check=False)
    if result.returncode != 0:
        if lfd_process is not None:
            _stop_process(lfd_process, "lfd")
        if lfd_log is not None:
            lfd_log.close()
        return result.returncode

    _install_dev_app()
    print("Logs: ~/Library/Logs/Loopflow/")
    print(f"Stream log: {LOOPFLOW_STREAM_LOG}")
    # The app resolves `lf` from its own bundle before PATH, so the dashboard
    # reads this branch's ledger surfaces (`lf runs/trace/doctor --json`) rather
    # than whatever `lf` happens to be installed.
    print(f"Bundled lf: {DEV_APP}/Contents/MacOS/lf")
    print("Telemetry dashboard: Go → Telemetry (⌘1)")
    _print_run_debug_checklist()
    print("Press Ctrl+C to quit")
    print("---")
    app_exit = run_app_bundle_with_log(
        DEV_APP,
        LOOPFLOW_STREAM_LOG,
        args=["--repo", str(repo)],
        # Dev launches read the launched checkout's wave/ dir as-is; a plain
        # production launch leaves this unset and reads the main worktree.
        env={"LOOPFLOW_DEV_WAVE_REPO": str(repo)},
    )

    if lfd_process is not None:
        _stop_process(lfd_process, "lfd")
    if lfd_log is not None:
        lfd_log.close()
    return app_exit


def cmd_release() -> int:
    """Build release .app and .dmg. Delegates to scripts/release-loopflow.py."""
    script = REPO_ROOT / "scripts" / "release-loopflow.py"
    result = run([sys.executable, str(script)], check=False)
    return result.returncode


def cmd_clean() -> int:
    """Remove dev app and reset permissions."""
    print("Removing dev app...")
    if DEV_APP.exists():
        shutil.rmtree(DEV_APP)

    print("Resetting Accessibility permissions...")
    run(["tccutil", "reset", "Accessibility", "com.loopflow.mac"], check=False)
    print("Resetting Automation permissions...")
    run(["tccutil", "reset", "AppleEvents", "com.loopflow.mac"], check=False)
    print("Resetting Microphone permissions...")
    run(["tccutil", "reset", "Microphone", "com.loopflow.mac"], check=False)
    print("Done. Next run will require re-granting permissions.")
    return 0


def cmd_xcode() -> int:
    """Open in Xcode."""
    print("Opening in Xcode...")
    return run(["open", str(SWIFT_DIR / "LoopflowSwift.xcodeproj")], check=False).returncode


def cmd_logs() -> int:
    """Tail the app logs."""
    log_dir = Path.home() / "Library" / "Logs" / "Loopflow"
    if log_dir.exists():
        logs = list(log_dir.glob("*.log"))
        if logs:
            return run(["tail", "-f"] + [str(log) for log in logs], check=False).returncode
    print("No logs yet. Run the app first.")
    return 1


def cmd_lfd(kill: bool = False) -> int:
    """Stop installed lfd and run from this branch."""
    _stop_installed_lfd()

    if kill:
        print("Preflight kill complete; starting lfd from this branch...")

    return _lfd_native()


def _stop_installed_lfd() -> None:
    """Stop any running lfd (launchd, pid file, port)."""
    label = "com.loopflow.lfd"
    plist = Path.home() / "Library" / "LaunchAgents" / f"{label}.plist"
    domain_label = f"gui/{os.getuid()}/{label}"

    # bootout fully detaches the job from launchd ("spawn scheduled" can keep
    # clobbering ~/.lf/session-token even when the daemon itself fails).
    launchd_status = run_capture(["launchctl", "print", domain_label])
    if launchd_status.returncode == 0:
        print("Stopping lfd launchd service...")
        run(["launchctl", "bootout", domain_label], check=False)

    # Some environments can keep the label registered after bootout.
    if run_capture(["launchctl", "print", domain_label]).returncode == 0:
        print("Forcing launchd label removal for lfd...")
        run(["launchctl", "remove", label], check=False)

    if plist.exists():
        disabled = plist.with_suffix(".plist.disabled")
        if disabled.exists():
            print(f"Removing active launchd plist at {plist} (already disabled once)...")
            plist.unlink(missing_ok=True)
        else:
            print(f"Disabling launchd plist at {plist}...")
            plist.rename(disabled)

    pid_file = Path.home() / ".lf" / "lfd.pid"
    if pid_file.exists():
        try:
            pid = int(pid_file.read_text().strip())
            print(f"Stopping lfd (pid {pid})...")
            os.kill(pid, signal.SIGTERM)
            time.sleep(1.0)
        except (ValueError, ProcessLookupError):
            pass
        pid_file.unlink(missing_ok=True)

    _stop_lfd_on_port(2486)


def _lfd_native() -> int:
    """Build and run lfd natively (sqlite, local executor)."""
    env = os.environ.copy()
    env["GRPC_ENABLE_FORK_SUPPORT"] = "0"
    env["GRPC_VERBOSITY"] = "ERROR"
    # Isolate sqlite state per repo to avoid schema drift across sibling repos.
    env.setdefault("LFD_DB_PATH", f"lfd-{REPO_ROOT.name}.db")

    print("Building lfd...")
    result = run(["cargo", "build", "--locked", "--bin", "lfd"], cwd=REPO_ROOT, check=False)
    if result.returncode != 0:
        return result.returncode

    env["RUST_LOG"] = "loopflow=debug,tower_http=debug"

    lfd_bin = str(REPO_ROOT / "target" / "debug" / "lfd")
    print(f"Using sqlite DB: ~/.lf/{env['LFD_DB_PATH']}")
    print(f"Stream log: {LFD_STREAM_LOG}")
    print("Starting lfd from this branch (debug logging enabled)...")
    return stream_with_log([lfd_bin, "serve"], env=env, log_path=LFD_STREAM_LOG)


def _ensure_agent_image() -> int:
    """Build the agent image if it doesn't exist."""
    result = run_capture(["docker", "image", "inspect", "loopflow/agent:latest"])
    if result.returncode == 0:
        print("Agent image exists (loopflow/agent:latest)")
        return 0
    return cmd_agent_image()


def cmd_agent_image() -> int:
    """Build the Docker agent image."""
    print("Building loopflow/agent:latest...")
    return run(
        ["docker", "build", "-t", "loopflow/agent:latest", "docker/agent"],
        cwd=REPO_ROOT,
        check=False,
    ).returncode


def _stop_lfd_on_port(port: int) -> None:
    """Stop any local process listening on the given port and print wave context."""
    pids = _list_pids_on_port(port)
    if not pids:
        return

    for pid in pids:
        print(_describe_lfd_process(pid, port))

    for pid in pids:
        try:
            print(f"Stopping local process {pid} on port {port}...")
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass

    time.sleep(1.0)

    for pid in _list_pids_on_port(port):
        try:
            print(f"Force-killing local process {pid} on port {port}...")
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass


def _list_pids_on_port(port: int) -> list[int]:
    result = run_capture(["lsof", "-nP", "-iTCP:" + str(port), "-sTCP:LISTEN", "-t"])
    if result.returncode != 0 or not result.stdout.strip():
        return []

    pids: list[int] = []
    for raw in result.stdout.strip().splitlines():
        try:
            pids.append(int(raw.strip()))
        except ValueError:
            continue
    return sorted(set(pids))


def _describe_lfd_process(pid: int, port: int) -> str:
    cmd = run_capture(["ps", "-p", str(pid), "-o", "command="]).stdout.strip()
    repo = _infer_repo_for_pid(pid, cmd)
    branch = _git_branch(repo) if repo else None
    wave = _wave_name_from_branch(branch) if branch else None

    parts = [f"Found local process on :{port}", f"pid={pid}"]
    if wave:
        parts.append(f"wave={wave}")
    if branch:
        parts.append(f"branch={branch}")
    if repo:
        parts.append(f"repo={repo}")
    if cmd:
        parts.append(f"cmd={cmd}")
    return " | ".join(parts)


def _infer_repo_for_pid(pid: int, cmd: str) -> str | None:
    cwd = _cwd_for_pid(pid)
    if cwd and (Path(cwd) / ".git").exists():
        return cwd

    if not cmd:
        return None
    executable = cmd.split()[0]
    try:
        path = Path(executable).resolve()
    except OSError:
        return None

    if path.name != "lfd":
        return None
    # .../<repo>/target/{debug,release}/lfd
    if len(path.parents) >= 3 and path.parents[1].name == "target":
        repo = path.parents[2]
        if (repo / ".git").exists():
            return str(repo)
    return None


def _cwd_for_pid(pid: int) -> str | None:
    result = run_capture(["lsof", "-a", "-p", str(pid), "-d", "cwd", "-Fn"])
    if result.returncode != 0 or not result.stdout.strip():
        return None
    for line in result.stdout.splitlines():
        if line.startswith("n"):
            return line[1:]
    return None


def _git_branch(repo: str) -> str | None:
    result = run_capture(["git", "-C", repo, "branch", "--show-current"])
    branch = result.stdout.strip()
    if result.returncode != 0 or not branch:
        return None
    return branch


def _wave_name_from_branch(branch: str) -> str | None:
    if branch.startswith("jack-heart."):
        parts = branch.split(".")
        if len(parts) > 1 and parts[1]:
            return parts[1]
    if branch.startswith("wave/"):
        wave = branch.removeprefix("wave/").split(".")[0]
        return wave or None
    return None


def _find_stable_signing_identity() -> str | None:
    """Pick a stable codesigning identity already in the keychain.

    Prefers our dedicated dev cert, then a real Apple identity (what Xcode uses
    for local debug builds). Any of these keeps the cdhash constant across
    builds so the connection-token ACL persists. Returns None if none exist.
    """
    result = run_capture(["security", "find-identity", "-v", "-p", "codesigning"])
    names = []
    for line in result.stdout.splitlines():
        start = line.find('"')
        end = line.rfind('"')
        if start != -1 and end > start:
            names.append(line[start + 1 : end])

    for preferred in (DEV_SIGNING_IDENTITY, "Apple Development", "Developer ID Application"):
        for name in names:
            if name.startswith(preferred):
                return name
    return names[0] if names else None


def _create_dev_signing_identity() -> None:
    """Create a stable, trusted self-signed code-signing identity.

    Fallback for machines with no Apple developer identity. The cert must be
    trusted for code signing or `codesign` and `find-identity` won't accept it;
    the trust skill may raise a one-time authorization dialog.
    """
    print(f'Creating stable dev signing identity "{DEV_SIGNING_IDENTITY}"...')
    p12_password = "loopflow-dev"  # transient; only unlocks the export archive
    with tempfile.TemporaryDirectory() as tmp:
        tmp_dir = Path(tmp)
        config = tmp_dir / "cert.conf"
        key = tmp_dir / "key.pem"
        cert = tmp_dir / "cert.pem"
        p12 = tmp_dir / "identity.p12"
        config.write_text(
            "[req]\n"
            "distinguished_name = dn\n"
            "x509_extensions = v3\n"
            "prompt = no\n"
            "[dn]\n"
            f"CN = {DEV_SIGNING_IDENTITY}\n"
            "[v3]\n"
            "basicConstraints = critical,CA:false\n"
            "keyUsage = critical,digitalSignature\n"
            "extendedKeyUsage = critical,codeSigning\n"
        )
        run(
            [
                "openssl",
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-keyout",
                str(key),
                "-out",
                str(cert),
                "-days",
                "3650",
                "-config",
                str(config),
                "-extensions",
                "v3",
            ]
        )
        # An empty p12 password trips a MAC-verification failure in macOS'
        # `security import`, so use a transient one.
        run(
            [
                "openssl",
                "pkcs12",
                "-export",
                "-inkey",
                str(key),
                "-in",
                str(cert),
                "-out",
                str(p12),
                "-name",
                DEV_SIGNING_IDENTITY,
                "-passout",
                f"pass:{p12_password}",
            ]
        )
        # -A lets any app (incl. codesign) use the private key without an access
        # prompt. Importing into the already-unlocked login keychain is
        # non-interactive.
        run(
            [
                "security",
                "import",
                str(p12),
                "-k",
                str(LOGIN_KEYCHAIN),
                "-P",
                p12_password,
                "-A",
            ]
        )
        # Trust the cert for code signing so codesign/find-identity accept it.
        run(
            [
                "security",
                "add-trusted-cert",
                "-r",
                "trustRoot",
                "-p",
                "codeSign",
                "-k",
                str(LOGIN_KEYCHAIN),
                str(cert),
            ]
        )


def _ensure_dev_signing_identity() -> str:
    identity = _find_stable_signing_identity()
    if identity is None:
        _create_dev_signing_identity()
        identity = _find_stable_signing_identity()
    if identity is None:
        raise RuntimeError("Failed to create a dev signing identity")
    return identity


def _bundle_executable_name(info_plist: Path) -> str:
    """The binary macOS actually launches, per CFBundleExecutable."""
    result = run_capture(
        ["/usr/libexec/PlistBuddy", "-c", "Print :CFBundleExecutable", str(info_plist)]
    )
    name = (result.stdout or "").strip()
    if result.returncode != 0 or not name:
        raise RuntimeError(f"Could not read CFBundleExecutable from {info_plist}")
    return name


def _install_dev_app() -> None:
    """Install debug build to stable location for permissions persistence."""
    app_dir = DEV_APP / "Contents"
    macos_dir = app_dir / "MacOS"
    macos_dir.mkdir(parents=True, exist_ok=True)
    (app_dir / "Resources").mkdir(parents=True, exist_ok=True)

    # `swift build` names the product LoopflowMac, but Info.plist declares
    # CFBundleExecutable = Loopflow, so that is the name macOS launches. Copying
    # the build under its own name leaves whatever `Loopflow` was already there
    # to run forever: the app keeps starting a stale binary while the fresh one
    # sits beside it, and codesigning bumps the stale mtime so it even looks new.
    executable = _bundle_executable_name(SWIFT_DIR / "LoopflowMac" / "Info.plist")
    shutil.copy(SWIFT_DIR / ".build" / "debug" / "LoopflowMac", macos_dir / executable)
    stale = macos_dir / "LoopflowMac"
    if stale.name != executable and stale.exists():
        stale.unlink()
    shutil.copy(SWIFT_DIR / "LoopflowMac" / "Info.plist", app_dir)
    _apply_dev_identity(app_dir / "Info.plist")
    shutil.copy(SWIFT_DIR / "LoopflowMac" / "Loopflow.sdef", app_dir / "Resources")
    shutil.copy(SWIFT_DIR / "LoopflowMac" / "AppIcon.icns", app_dir / "Resources")
    _copy_bundled_tools(app_dir / "MacOS", profile="debug")

    identity = _ensure_dev_signing_identity()
    entitlements = SWIFT_DIR / "LoopflowMac" / "Loopflow.entitlements"
    codesign_cmd = ["codesign", "--force", "--deep", "--sign", identity]
    if entitlements.exists():
        codesign_cmd += ["--entitlements", str(entitlements)]
    codesign_cmd.append(str(DEV_APP))
    run(codesign_cmd)


def _apply_dev_identity(plist: Path) -> None:
    """Rewrite the dev app's bundle id and name so it gets its own settings domain."""
    run(["plutil", "-replace", "CFBundleIdentifier", "-string", DEV_BUNDLE_ID, str(plist)])
    run(["plutil", "-replace", "CFBundleName", "-string", "Loopflow Dev", str(plist)])
    run(["plutil", "-replace", "CFBundleDisplayName", "-string", "Loopflow Dev", str(plist)])


def _copy_bundled_tools(app_macos_dir: Path, profile: str) -> None:
    if profile == "release":
        cargo_cmd = ["cargo", "build", "--locked", "--release", "--bin", "lf"]
        bin_dir = REPO_ROOT / "target" / "release"
    else:
        cargo_cmd = ["cargo", "build", "--locked", "--bin", "lf"]
        bin_dir = REPO_ROOT / "target" / "debug"

    result = run(cargo_cmd, cwd=REPO_ROOT, check=False)
    if result.returncode != 0:
        raise RuntimeError("Failed to build bundled lf binary")

    for binary in ("lf",):
        source = bin_dir / binary
        if not source.exists():
            raise RuntimeError(f"Missing built binary: {source}")
        shutil.copy(source, app_macos_dir / binary)


# --- Screenshots ---


def cmd_screenshots() -> int:
    """Generate app screenshots."""
    script = REPO_ROOT / "scripts" / "generate_screenshots.py"
    result = subprocess.run(
        [sys.executable, str(script)],
        cwd=REPO_ROOT,
    )
    return result.returncode


# --- Main ---


COMMANDS = {
    "setup": (cmd_setup, "Install/check repo dev environment tools"),
    "build": (cmd_build, "Build the app"),
    "test": (cmd_test, "Build and run tests"),
    "run": (cmd_run, "Build and launch the app"),
    "run-debug": (cmd_run_debug, "Build and run with stdout visible"),
    "release": (cmd_release, "Build release .app and .dmg"),
    "clean": (cmd_clean, "Remove dev app and reset permissions"),
    "xcode": (cmd_xcode, "Open in Xcode"),
    "logs": (cmd_logs, "Tail the app logs"),
    "lfd": (cmd_lfd, "Stop local lfd and run from this branch (-k for preflight kill)"),
    "agent-image": (cmd_agent_image, "Build the Docker agent image"),
    "screenshots": (cmd_screenshots, "Generate app screenshots"),
}


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Development commands for Loopflow",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    subparsers = parser.add_subparsers(dest="command", metavar="command")

    for name, (func, help_text) in COMMANDS.items():
        sub = subparsers.add_parser(name, help=help_text)
        if name == "lfd":
            sub.add_argument(
                "-k",
                "--kill",
                action="store_true",
                help="Aggressive preflight kill (default also kills port 2486 listeners)",
            )
        if name == "run-debug":
            sub.add_argument(
                "--with-lfd",
                action="store_true",
                help="Also run one-shot lfd lifecycle from this branch",
            )
            sub.add_argument(
                "--repo",
                type=lambda p: Path(p).expanduser().resolve(),
                default=REPO_ROOT,
                help="Repo the app opens on launch (default: this checkout)",
            )
        if name == "setup":
            sub.add_argument(
                "--install",
                action="store_true",
                help="install missing tools (default is check-only)",
            )
            sub.add_argument(
                "--dry-run",
                action="store_true",
                help="print what would be installed",
            )
    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        return 0

    func, _ = COMMANDS[args.command]
    if args.command == "lfd":
        return func(kill=args.kill)
    if args.command == "run-debug":
        with_lfd = args.with_lfd
        return func(with_lfd=with_lfd, repo=args.repo)
    if args.command == "setup":
        return func(install=args.install, dry_run=args.dry_run)
    return func()


if __name__ == "__main__":
    sys.exit(main())
