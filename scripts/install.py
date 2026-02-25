#!/usr/bin/env python3
"""Build and install loopflow locally.

install.py local            # build and install everything
install.py local --service  # also restart lfd service

Remote releases happen via `lf release patch` → merge → auto-tag → CI.
"""

import os
import shutil
import subprocess
from pathlib import Path

import typer

ROOT = Path(__file__).parent.parent
DEFAULT_INSTALL_DIR = Path.home() / ".local" / "bin"

app = typer.Typer(help="Build and install loopflow locally.")


# --- Build helpers ---


def _build_wheel() -> tuple[bool, str]:
    result = subprocess.run(
        ["uv", "build"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0, result.stdout + result.stderr


def _install_wheel() -> tuple[bool, str]:
    wheels = sorted((ROOT / "dist").glob("loopflow-*.whl"))
    if not wheels:
        return False, "No wheel found in dist/"
    wheel = str(wheels[-1])

    # Install into local venv (uv run lfq)
    result = subprocess.run(
        ["uv", "pip", "install", "--force-reinstall", wheel],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return False, result.stdout + result.stderr

    # Install as global tool (lfq on PATH)
    result = subprocess.run(
        ["uv", "tool", "install", "--force", "--reinstall", wheel],
        capture_output=True,
        text=True,
    )
    return result.returncode == 0, result.stdout + result.stderr


def _install_binaries() -> tuple[bool, str, Path | None]:
    install_path = _resolve_install_dir()
    install_path.mkdir(parents=True, exist_ok=True)

    result = subprocess.run(
        ["cargo", "build", "-p", "loopflow", "--release"],
        cwd=ROOT,
    )
    if result.returncode != 0:
        return False, "cargo build failed (see output above)", None

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


def _install_concerto() -> tuple[bool, str]:
    swift_dir = ROOT / "swift"
    if not swift_dir.exists():
        return False, f"swift directory not found: {swift_dir}"

    result = subprocess.run(
        ["swift", "build", "-c", "release"],
        cwd=swift_dir,
    )
    if result.returncode != 0:
        return False, "swift build -c release failed (see output above)"

    app_name = "Loopflow Concerto"
    app_path = Path("/Applications") / f"{app_name}.app"
    app_dir = app_path / "Contents"

    # Clean existing install
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


# --- Service management ---


def _restart_lfd() -> None:
    label = "com.loopflow.lfd"
    plist = Path.home() / "Library" / "LaunchAgents" / f"{label}.plist"

    lfd_bin = shutil.which("lfd")
    if lfd_bin:
        typer.echo("Reinstalling lfd service (updates PATH)...")
        result = subprocess.run([lfd_bin, "install"], capture_output=True, text=True, timeout=15)
        if result.returncode == 0:
            typer.echo("lfd service restarted.")
            return
        typer.echo(f"lfd install failed, falling back to launchctl: {result.stderr.strip()}")

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
        typer.echo("Would build wheel with uv")
        typer.echo("Would install wheel with uv pip install")
        typer.echo("Would install lf/lfd binaries")
        typer.echo("Would install Concerto to /Applications")
        if service:
            typer.echo("Would install/restart lfd service")
        else:
            typer.echo("Would NOT install/restart lfd service (pass --service to enable)")
        return

    typer.echo("Building wheel...")
    ok, output = _build_wheel()
    if not ok:
        typer.echo(f"Build failed:\n{output}", err=True)
        raise typer.Exit(code=1)
    typer.echo("Build succeeded.")

    typer.echo("Installing locally...")
    ok, output = _install_wheel()
    if not ok:
        typer.echo(f"Install failed:\n{output}", err=True)
        raise typer.Exit(code=1)
    typer.echo("Installed.")

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

    typer.echo("Building and installing Concerto...")
    ok, output = _install_concerto()
    if not ok:
        typer.echo(f"Concerto install failed:\n{output}", err=True)
    else:
        typer.echo(output)

    if service:
        _restart_lfd()
    else:
        typer.echo("Skipping lfd service install/restart. Pass --service to enable.")


if __name__ == "__main__":
    app()
