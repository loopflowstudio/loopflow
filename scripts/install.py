#!/usr/bin/env python3
"""Build and install loopflow locally.

install.py local            # build and install everything
install.py local --service  # also restart lfd service

Remote releases happen via `lf release patch` -> merge -> auto-tag -> CI.
"""

import os
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

import typer

ROOT = Path(__file__).parent.parent
DEFAULT_INSTALL_DIR = Path.home() / ".local" / "bin"

app = typer.Typer(help="Build and install loopflow locally.")


# --- Streaming output ---


def _stream_process(
    cmd: list[str],
    label: str,
    cwd: Path | None = None,
) -> subprocess.CompletedProcess[bytes]:
    """Run a subprocess, streaming its output with a label prefix."""
    proc = subprocess.Popen(
        cmd,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    assert proc.stdout is not None
    for line in proc.stdout:
        text = line.decode("utf-8", errors="replace").rstrip()
        print(f"  [{label}] {text}", flush=True)
    proc.wait()
    return subprocess.CompletedProcess(cmd, proc.returncode)


# --- Build helpers ---


def _build_wheel() -> tuple[bool, str]:
    typer.echo("Building wheel...")
    result = _stream_process(["uv", "build"], "wheel", cwd=ROOT)
    if result.returncode != 0:
        return False, "uv build failed (see output above)"
    return True, "Wheel built."


def _install_wheel() -> tuple[bool, str]:
    wheels = sorted((ROOT / "dist").glob("loopflow-*.whl"))
    if not wheels:
        return False, "No wheel found in dist/"
    wheel = str(wheels[-1])

    typer.echo("Installing wheel into local venv...")
    result = _stream_process(
        ["uv", "pip", "install", "--force-reinstall", wheel], "pip"
    )
    if result.returncode != 0:
        return False, "uv pip install failed (see output above)"

    typer.echo("Installing wheel as global tool...")
    result = _stream_process(
        ["uv", "tool", "install", "--force", "--reinstall", wheel], "tool"
    )
    if result.returncode != 0:
        return False, "uv tool install failed (see output above)"
    return True, "Wheel installed."


def _build_binaries() -> tuple[bool, str]:
    """Cargo release build only (no install)."""
    typer.echo("Building lf/lfd (cargo release)...")
    result = _stream_process(
        ["cargo", "build", "-p", "loopflow", "--release"], "cargo", cwd=ROOT
    )
    if result.returncode != 0:
        return False, "cargo build failed (see output above)"
    return True, "Cargo build succeeded."


def _install_binaries() -> tuple[bool, str, Path | None]:
    """Copy pre-built binaries to install dir."""
    install_path = _resolve_install_dir()
    install_path.mkdir(parents=True, exist_ok=True)

    built = ROOT / "target" / "release"
    for name in ("lf", "lfd"):
        src = built / name
        if src.exists():
            dst = install_path / name
            _install_binary(src, dst)

    return True, f"Installed lf/lfd to {install_path}", install_path


def _resolve_install_dir() -> Path:
    env_dir = os.environ.get("LF_INSTALL_DIR")
    if env_dir:
        return Path(env_dir).expanduser()

    existing = shutil.which("lf")
    if existing:
        existing_path = Path(existing).expanduser()
        parent = existing_path.parent
        user_dirs = {
            (Path.home() / ".local" / "bin").resolve(),
            (Path.home() / ".lf" / "bin").resolve(),
        }
        try:
            parent_resolved = parent.resolve()
            if parent_resolved in user_dirs and _is_writable(parent):
                return parent
            if (
                parent_resolved == Path("/usr/local/bin")
                and _is_writable(parent)
                and _looks_like_loopflow(existing_path)
            ):
                return parent
        except OSError:
            pass

    return DEFAULT_INSTALL_DIR


def _is_writable(path: Path) -> bool:
    return path.exists() and path.is_dir() and os.access(path, os.W_OK)


def _looks_like_loopflow(binary: Path) -> bool:
    try:
        result = subprocess.run(
            [str(binary), "--help"],
            capture_output=True,
            text=True,
            timeout=2,
        )
    except (subprocess.SubprocessError, OSError):
        return False
    return "Run steps and flows with coding agents" in result.stdout


def _install_binary(src: Path, dst: Path) -> None:
    tmp = dst.with_name(f".{dst.name}.tmp.{os.getpid()}")
    try:
        shutil.copyfile(src, tmp)
        tmp.chmod(0o755)
        tmp.replace(dst)
    finally:
        if tmp.exists():
            tmp.unlink(missing_ok=True)


def _build_concerto() -> tuple[bool, str]:
    """Swift release build only (no install)."""
    swift_dir = ROOT / "swift"
    if not swift_dir.exists():
        return False, f"swift directory not found: {swift_dir}"

    typer.echo("Building Concerto (swift release)...")
    result = _stream_process(
        ["swift", "build", "-c", "release"], "swift", cwd=swift_dir
    )
    if result.returncode != 0:
        return False, "swift build -c release failed (see output above)"
    return True, "Swift build succeeded."


def _install_concerto() -> tuple[bool, str]:
    """Install pre-built Concerto into /Applications."""
    swift_dir = ROOT / "swift"
    app_name = "Loopflow Concerto"
    app_path = Path("/Applications") / f"{app_name}.app"
    app_dir = app_path / "Contents"

    if app_path.exists():
        shutil.rmtree(app_path)

    (app_dir / "MacOS").mkdir(parents=True)
    (app_dir / "Resources").mkdir(parents=True)

    shutil.copy(swift_dir / ".build" / "release" / "Concerto", app_dir / "MacOS")
    shutil.copy(swift_dir / "Concerto" / "Info.plist", app_dir)
    shutil.copy(swift_dir / "Concerto" / "Concerto.sdef", app_dir / "Resources")
    shutil.copy(swift_dir / "Concerto" / "AppIcon.icns", app_dir / "Resources")
    (app_dir / "PkgInfo").write_text("APPL????")

    result = subprocess.run(
        ["codesign", "--force", "--deep", "--sign", "-", str(app_path)],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return False, f"codesign failed: {result.stderr}"

    return True, f"Installed {app_path}"


# --- Parallel build runner ---


def _run_parallel_builds() -> dict[str, tuple[bool, str]]:
    """Run wheel, cargo, and swift builds in parallel. Returns results keyed by name."""
    results: dict[str, tuple[bool, str]] = {}
    lock = threading.Lock()

    def _run(name: str, fn):
        start = time.monotonic()
        try:
            result = fn()
        except Exception as e:
            result = (False, f"Exception: {e}")
        elapsed = time.monotonic() - start
        with lock:
            results[name] = result
            status = "done" if result[0] else "FAILED"
            typer.echo(f"\n>>> {name} {status} ({elapsed:.1f}s)")

    threads = [
        threading.Thread(target=_run, args=("wheel", _build_wheel)),
        threading.Thread(target=_run, args=("cargo", _build_binaries)),
        threading.Thread(target=_run, args=("swift", _build_concerto)),
    ]

    typer.echo("Starting parallel builds: wheel, cargo, swift\n")
    for t in threads:
        t.start()
    for t in threads:
        t.join()

    typer.echo("")
    return results


# --- Service management ---


def _restart_lfd() -> None:
    label = "com.loopflow.lfd"
    plist = Path.home() / "Library" / "LaunchAgents" / f"{label}.plist"

    lfd_bin = shutil.which("lfd")
    if lfd_bin:
        typer.echo("Reinstalling lfd service (updates PATH)...")
        result = subprocess.run(
            [lfd_bin, "install"], capture_output=True, text=True, timeout=15
        )
        if result.returncode == 0:
            typer.echo("lfd service restarted.")
            return
        typer.echo(
            f"lfd install failed, falling back to launchctl: {result.stderr.strip()}"
        )

    if not plist.exists():
        typer.echo("lfd launchd plist not found, skipping restart.")
        return

    typer.echo("Restarting lfd...")
    subprocess.run(["launchctl", "unload", str(plist)], capture_output=True, timeout=10)
    subprocess.run(["launchctl", "load", str(plist)], capture_output=True, timeout=10)
    typer.echo("lfd restarted.")


# --- Commands ---


@app.command()
def local(
    dry_run: bool = typer.Option(False, "-n", "--dry-run", help="Show what would be done"),
    service: bool = typer.Option(
        False,
        "--service",
        help="Install/restart lfd as a launchd service after local install",
    ),
):
    """Build and install locally (no publish)."""
    if dry_run:
        typer.echo("Would build wheel, cargo, and swift in parallel")
        typer.echo("Would install wheel with uv pip install + uv tool install")
        typer.echo("Would install lf/lfd binaries")
        typer.echo("Would install Concerto to /Applications")
        if service:
            typer.echo("Would install/restart lfd service")
        else:
            typer.echo("Would NOT install/restart lfd service (pass --service to enable)")
        return

    total_start = time.monotonic()

    # Phase 1: parallel builds
    build_results = _run_parallel_builds()

    failed = [name for name, (ok, _) in build_results.items() if not ok]
    if failed:
        typer.echo(f"Builds failed: {', '.join(failed)}", err=True)
        for name in failed:
            typer.echo(f"  {name}: {build_results[name][1]}", err=True)
        raise typer.Exit(code=1)

    # Phase 2: sequential installs (fast, just copying files)
    typer.echo("Installing wheel...")
    ok, output = _install_wheel()
    if not ok:
        typer.echo(f"Wheel install failed:\n{output}", err=True)
        raise typer.Exit(code=1)
    typer.echo(output)

    typer.echo("Installing lf/lfd binaries...")
    ok, output, install_path = _install_binaries()
    if not ok:
        typer.echo(f"Binary install failed:\n{output}", err=True)
        raise typer.Exit(code=1)
    typer.echo(output)

    install_path = install_path or DEFAULT_INSTALL_DIR
    for name in ("lf", "lfd"):
        result = subprocess.run(["which", name], capture_output=True, text=True)
        if result.returncode == 0:
            resolved = Path(result.stdout.strip())
            expected = install_path / name
            typer.echo(f"{name}: {resolved}")
            if resolved != expected:
                typer.echo(
                    f"Note: {name} resolves to {resolved}, not {expected}. "
                    f"Add {install_path} to PATH to use the freshly installed binary.",
                    err=True,
                )

    typer.echo("Installing Concerto...")
    ok, output = _install_concerto()
    if not ok:
        typer.echo(f"Concerto install failed:\n{output}", err=True)
    else:
        typer.echo(output)

    if service:
        _restart_lfd()
    else:
        typer.echo("Skipping lfd service install/restart. Pass --service to enable.")

    elapsed = time.monotonic() - total_start
    typer.echo(f"\nTotal install time: {elapsed:.1f}s")


if __name__ == "__main__":
    app()
