#!/usr/bin/env python3
"""Build and install loopflow locally.

    install.py                  # build and install everything
    install.py --service        # also install/restart lfd as a launchd service
    install.py --skip wheel     # skip a build stage (wheel|cargo|swift)
    install.py -n               # dry run

Remote releases happen via `lf release patch` -> merge -> auto-tag -> CI.
"""

from __future__ import annotations

import os
import platform
import shutil
import subprocess
import threading
import time
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

import typer

ROOT = Path(__file__).parent.parent
DEFAULT_INSTALL_DIR = Path.home() / ".local" / "bin"
APPLICATIONS_DIR = Path("/Applications")
BUILD_STAGES = ("wheel", "cargo", "swift")


# --- Bundle spec (single source of truth for Concerto.app layout) ---


@dataclass(frozen=True)
class BundleSpec:
    """Paths that define the Concerto.app bundle."""

    app_path: Path
    executables: tuple[Path, ...]
    info_plist: Path
    resources: tuple[Path, ...]

    @property
    def contents_dir(self) -> Path:
        return self.app_path / "Contents"

    @property
    def macos_dir(self) -> Path:
        return self.contents_dir / "MacOS"

    @property
    def resources_dir(self) -> Path:
        return self.contents_dir / "Resources"

    def macos_binaries(self) -> tuple[Path, ...]:
        return tuple(self.macos_dir / executable.name for executable in self.executables)


def default_bundle_spec(
    root: Path = ROOT,
    applications_dir: Path = APPLICATIONS_DIR,
) -> BundleSpec:
    swift = root / "swift"
    cargo_release = root / "target" / "release"
    return BundleSpec(
        app_path=applications_dir / "Loopflow Concerto.app",
        executables=(
            swift / ".build" / "release" / "Concerto",
            cargo_release / "lf",
            cargo_release / "lfd",
        ),
        info_plist=swift / "Concerto" / "Info.plist",
        resources=(
            swift / "Concerto" / "Concerto.sdef",
            swift / "Concerto" / "AppIcon.icns",
        ),
    )


# --- Errors and subprocess helpers ---


class StageError(RuntimeError):
    """A build or install stage failed."""


def _stream_process(cmd: list[str], label: str, cwd: Path | None = None) -> int:
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
    return proc.returncode


def _run_or_raise(cmd: list[str], label: str, cwd: Path | None = None) -> None:
    code = _stream_process(cmd, label, cwd=cwd)
    if code != 0:
        raise StageError(f"{label} exited {code}")


# --- Builds ---


def _build_wheel() -> None:
    typer.echo("Building wheel...")
    _run_or_raise(["uv", "build", "--wheel"], "wheel", cwd=ROOT)


def _build_binaries() -> None:
    typer.echo("Building lf/lfd (cargo release)...")
    _run_or_raise(["cargo", "build", "-p", "loopflow", "--release"], "cargo", cwd=ROOT)


def _build_concerto() -> None:
    swift_dir = ROOT / "swift"
    if not swift_dir.exists():
        raise StageError(f"swift directory not found: {swift_dir}")
    typer.echo("Building Concerto (swift release)...")
    _run_or_raise(["swift", "build", "-c", "release"], "swift", cwd=swift_dir)


def _run_parallel_builds(skip: set[str]) -> None:
    stages = {"wheel": _build_wheel, "cargo": _build_binaries, "swift": _build_concerto}
    active = {name: fn for name, fn in stages.items() if name not in skip}
    if not active:
        typer.echo("All build stages skipped.")
        return

    errors: dict[str, str] = {}
    lock = threading.Lock()

    def _run(name: str, fn: Callable[[], None]) -> None:
        start = time.monotonic()
        status = "done"
        try:
            fn()
        except Exception as exc:
            with lock:
                errors[name] = str(exc)
            status = "FAILED"
        elapsed = time.monotonic() - start
        with lock:
            typer.echo(f"\n>>> {name} {status} ({elapsed:.1f}s)")

    typer.echo(f"Starting parallel builds: {', '.join(active)}\n")
    threads = [threading.Thread(target=_run, args=(n, fn)) for n, fn in active.items()]
    for t in threads:
        t.start()
    for t in threads:
        t.join()

    if errors:
        summary = "; ".join(f"{n}: {e}" for n, e in errors.items())
        raise StageError(f"builds failed: {summary}")


# --- Binary install ---


def _atomic_install(src: Path, dst: Path) -> None:
    if not src.exists():
        raise StageError(f"expected build artifact missing: {src}")
    tmp = dst.with_name(f".{dst.name}.tmp.{os.getpid()}")
    try:
        shutil.copyfile(src, tmp)
        tmp.chmod(0o755)
        tmp.replace(dst)
    except BaseException:
        tmp.unlink(missing_ok=True)
        raise


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


def _install_wheel() -> None:
    wheels = sorted((ROOT / "dist").glob("loopflow-*.whl"))
    if not wheels:
        raise StageError("no wheel found in dist/")
    wheel = str(wheels[-1])

    typer.echo("Installing wheel into local venv...")
    _run_or_raise(["uv", "pip", "install", "--force-reinstall", wheel], "pip")

    typer.echo("Installing wheel as global tool...")
    _run_or_raise(["uv", "tool", "install", "--force", "--reinstall", wheel], "tool")


def _install_binaries() -> Path:
    install_dir = _resolve_install_dir()
    install_dir.mkdir(parents=True, exist_ok=True)
    built = ROOT / "target" / "release"
    for name in ("lf", "lfd"):
        _atomic_install(built / name, install_dir / name)
    return install_dir


# --- Concerto bundle ---


def _install_concerto(spec: BundleSpec) -> None:
    if spec.app_path.exists():
        shutil.rmtree(spec.app_path)

    spec.macos_dir.mkdir(parents=True)
    spec.resources_dir.mkdir(parents=True)

    for executable in spec.executables:
        _atomic_install(executable, spec.macos_dir / executable.name)

    shutil.copy(spec.info_plist, spec.contents_dir / spec.info_plist.name)
    for resource in spec.resources:
        shutil.copy(resource, spec.resources_dir / resource.name)
    (spec.contents_dir / "PkgInfo").write_text("APPL????")

    _verify_bundle_layout(spec)

    sign = subprocess.run(
        ["codesign", "--force", "--deep", "--sign", "-", str(spec.app_path)],
        capture_output=True,
        text=True,
    )
    if sign.returncode != 0:
        detail = (sign.stderr or sign.stdout).strip()
        raise StageError(f"codesign failed: {detail}")

    _verify_bundle_signature(spec)


def _verify_bundle_layout(spec: BundleSpec) -> None:
    """Mirror Bundle.main.url(forAuxiliaryExecutable:) expectations.

    Checks every executable that Concerto will look up in Contents/MacOS/:
    it must exist, be executable, and be a Mach-O binary that includes the
    current architecture. Catches missing files, wrong-arch copies, and
    non-executable placeholders before we hand the bundle to codesign.
    """
    current_arch = platform.machine()
    problems: list[str] = []

    for path in spec.macos_binaries():
        if not path.exists():
            problems.append(f"missing: {path}")
            continue
        if not os.access(path, os.X_OK):
            problems.append(f"not executable: {path}")
            continue
        archs = _macho_archs(path)
        if not archs:
            problems.append(f"not a Mach-O binary: {path}")
            continue
        if current_arch not in archs:
            problems.append(f"{path} built for {', '.join(archs)}, not {current_arch}")

    for path in [spec.contents_dir / spec.info_plist.name] + [
        spec.resources_dir / r.name for r in spec.resources
    ]:
        if not path.exists():
            problems.append(f"missing resource: {path}")

    if problems:
        raise StageError("bundle verification failed: " + "; ".join(problems))


def _macho_archs(path: Path) -> list[str]:
    """Return the architectures in a Mach-O binary, or [] if not Mach-O."""
    result = subprocess.run(["lipo", "-archs", str(path)], capture_output=True, text=True)
    if result.returncode != 0:
        return []
    return result.stdout.split()


def _verify_bundle_signature(spec: BundleSpec) -> None:
    """Run `codesign --verify` as a signing smoke test."""
    result = subprocess.run(
        ["codesign", "--verify", "--verbose=4", str(spec.app_path)],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()
        raise StageError(f"codesign --verify failed: {detail}")
    # codesign writes diagnostics to stderr even on success; surface them.
    if result.stderr.strip():
        for line in result.stderr.splitlines():
            typer.echo(f"  [codesign] {line}")


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
        typer.echo(f"lfd launchd plist not found at {plist}, skipping restart.", err=True)
        return

    typer.echo("Restarting lfd via launchctl...")
    subprocess.run(["launchctl", "unload", str(plist)], capture_output=True, timeout=10)
    subprocess.run(["launchctl", "load", str(plist)], capture_output=True, timeout=10)
    typer.echo("lfd restarted.")


# --- Reporting ---


def _report_path_collisions(install_dir: Path) -> None:
    for name in ("lf", "lfd"):
        found = shutil.which(name)
        if not found:
            continue
        resolved = Path(found)
        expected = install_dir / name
        typer.echo(f"{name}: {resolved}")
        if resolved != expected:
            typer.echo(
                f"Note: {name} resolves to {resolved}, not {expected}. "
                f"Add {install_dir} to PATH to use the freshly installed binary.",
                err=True,
            )


# --- Commands ---


app = typer.Typer(help="Build and install loopflow locally.", add_completion=False)


@app.command()
def local(
    dry_run: bool = typer.Option(False, "-n", "--dry-run", help="Show what would be done"),
    service: bool = typer.Option(
        False, "--service", help="Install/restart lfd as a launchd service after local install"
    ),
    skip: list[str] = typer.Option(
        [], "--skip", help=f"Skip a build stage ({'|'.join(BUILD_STAGES)}); repeatable"
    ),
) -> None:
    """Build and install locally (no publish)."""
    skip_set = set(skip)
    unknown = skip_set - set(BUILD_STAGES)
    if unknown:
        raise typer.BadParameter(f"unknown --skip values: {', '.join(sorted(unknown))}")

    spec = default_bundle_spec()
    install_dir = _resolve_install_dir()

    if dry_run:
        planned = [s for s in BUILD_STAGES if s not in skip_set] or ["(nothing)"]
        typer.echo(f"Would build in parallel: {', '.join(planned)}")
        typer.echo("Would install wheel with uv pip install + uv tool install")
        typer.echo(f"Would install lf/lfd to {install_dir}")
        typer.echo(f"Would install {spec.app_path}")
        executable_names = ", ".join(path.name for path in spec.executables)
        typer.echo(f"  Contents/MacOS/: {executable_names}")
        typer.echo(
            "Would install/restart lfd service"
            if service
            else "Would skip lfd service install/restart (pass --service to enable)"
        )
        return

    total_start = time.monotonic()
    try:
        _run_parallel_builds(skip_set)

        typer.echo("Installing wheel...")
        _install_wheel()
        typer.echo("Wheel installed.")

        typer.echo(f"Installing lf/lfd to {install_dir}...")
        install_dir = _install_binaries()
        typer.echo(f"Installed lf/lfd to {install_dir}")
        _report_path_collisions(install_dir)

        typer.echo(f"Installing {spec.app_path}...")
        _install_concerto(spec)
        typer.echo(f"Installed {spec.app_path}")

        if service:
            _restart_lfd()
        else:
            typer.echo("Skipping lfd service install/restart. Pass --service to enable.")
    except StageError as exc:
        typer.echo(f"install failed: {exc}", err=True)
        raise typer.Exit(code=1) from exc

    elapsed = time.monotonic() - total_start
    typer.echo(f"\nTotal install time: {elapsed:.1f}s")


if __name__ == "__main__":
    app()
