#!/usr/bin/env python3
"""Install published Loopflow releases or build this worktree locally.

    install.py local            # build this worktree into local-bin/
    install.py local --skip swift
    install.py local -n         # dry run
    install.py refresh          # install the latest published release

Remote releases happen via `lf release patch` -> merge -> auto-tag -> CI.
"""

from __future__ import annotations

import hashlib
import os
import platform
import shutil
import subprocess
import tempfile
import threading
import time
import urllib.request
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

import typer
from bundle_version import read_release_version, stamp_bundle_version

ROOT = Path(__file__).parent.parent
DEFAULT_INSTALL_DIR = Path.home() / ".local" / "bin"
LOCAL_BIN = ROOT / "local-bin"
APP_NAME = "Loopflow"
# SwiftPM executable product; the library owns the bare `Loopflow` name, so the
# app binary is built as `LoopflowMac` and renamed to APP_NAME inside the bundle.
SWIFT_APP_PRODUCT = "LoopflowMac"
BUILD_STAGES = ("cargo", "swift")
LATEST_RELEASE_URL = "https://github.com/loopflowstudio/loopflow/releases/latest"
RELEASE_DOWNLOAD_BASE = "https://github.com/loopflowstudio/loopflow/releases/download"


# --- Bundle spec (single source of truth for Loopflow.app layout) ---


@dataclass(frozen=True)
class BundleSpec:
    """Paths that define the Loopflow.app bundle."""

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
        return tuple(
            self.macos_dir / _bundle_binary_name(executable) for executable in self.executables
        )


def _bundle_binary_name(executable: Path) -> str:
    """Name an executable takes inside the bundle: the app binary ships as
    APP_NAME (matching CFBundleExecutable); auxiliary tools keep their names."""
    if executable.name == SWIFT_APP_PRODUCT:
        return APP_NAME
    return executable.name


def default_bundle_spec(root: Path = ROOT) -> BundleSpec:
    swift = root / "swift"
    cargo_release = root / "target" / "release"
    return BundleSpec(
        app_path=root / "local-bin" / f"{APP_NAME}.app",
        executables=(
            swift / ".build" / "release" / "LoopflowMac",
            cargo_release / "lf",
            cargo_release / "lfd",
        ),
        info_plist=swift / "LoopflowMac" / "Info.plist",
        resources=(
            swift / "LoopflowMac" / "Loopflow.sdef",
            swift / "LoopflowMac" / "AppIcon.icns",
        ),
    )


# --- Errors and subprocess helpers ---


class StageError(RuntimeError):
    """A build or install stage failed."""


def _stream_process(
    cmd: list[str],
    label: str,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
) -> int:
    proc = subprocess.Popen(
        cmd,
        cwd=cwd,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    assert proc.stdout is not None
    for line in proc.stdout:
        text = line.decode("utf-8", errors="replace").rstrip()
        print(f"  [{label}] {text}", flush=True)
    proc.wait()
    return proc.returncode


def _run_or_raise(
    cmd: list[str],
    label: str,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
) -> None:
    code = _stream_process(cmd, label, cwd=cwd, env=env)
    if code != 0:
        raise StageError(f"{label} exited {code}")


# --- Builds ---


def _development_build_env() -> dict[str, str]:
    return {
        **os.environ,
        "LOOPFLOW_BUILD_PROVENANCE": "development",
        "LOOPFLOW_MIGRATION_AUTHORITY": "validation_only",
    }


def _build_binaries() -> None:
    typer.echo("Building lf (cargo release)...")
    _run_or_raise(
        ["cargo", "build", "-p", "loopflow", "--release"],
        "cargo",
        cwd=ROOT,
        env=_development_build_env(),
    )


def _build_loopflow() -> None:
    swift_dir = ROOT / "swift"
    if not swift_dir.exists():
        raise StageError(f"swift directory not found: {swift_dir}")
    typer.echo("Building Loopflow (swift release)...")
    _run_or_raise(["swift", "build", "-c", "release"], "swift", cwd=swift_dir)


def _run_parallel_builds(skip: set[str]) -> None:
    stages = {"cargo": _build_binaries, "swift": _build_loopflow}
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
    return "Run skills and flows with coding agents" in result.stdout


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


def _stage_binaries(local_bin: Path) -> None:
    """Copy freshly built control binaries into this worktree's local-bin/."""
    local_bin.mkdir(parents=True, exist_ok=True)
    _atomic_install(ROOT / "target" / "release" / "lf", local_bin / "lf")
    _atomic_install(ROOT / "target" / "release" / "lfd", local_bin / "lfd")


def _download_release_asset(url: str, destination: Path) -> str:
    try:
        with urllib.request.urlopen(url, timeout=30) as response:
            destination.write_bytes(response.read())
            return response.geturl()
    except OSError as exc:
        raise StageError(f"download failed: {url}: {exc}") from exc


def _release_tag_from_latest_url(url: str) -> str:
    marker = "/releases/tag/"
    if marker not in url:
        raise StageError(f"latest release did not resolve to a pinned tag: {url}")
    tag = url.split(marker, 1)[1].split("/", 1)[0]
    if not tag.startswith("v") or len(tag) == 1:
        raise StageError(f"latest release resolved to an invalid tag: {tag}")
    return tag


def _latest_release_tag() -> str:
    request = urllib.request.Request(LATEST_RELEASE_URL, method="HEAD")
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return _release_tag_from_latest_url(response.geturl())
    except OSError as exc:
        raise StageError(f"latest release lookup failed: {exc}") from exc


def _manifest_digest(manifest: Path, asset: str) -> str:
    for line in manifest.read_text().splitlines():
        fields = line.split()
        if len(fields) == 2 and fields[1].removeprefix("*") == asset:
            return fields[0]
    raise StageError(f"published SHA256SUMS does not name {asset}")


def _verify_release_asset(path: Path, expected: str) -> None:
    actual = hashlib.sha256(path.read_bytes()).hexdigest()
    if actual != expected:
        raise StageError(
            f"digest mismatch for {path.name}: expected {expected}, downloaded {actual}"
        )


def _install_published_release(install_dir: Path) -> str:
    with tempfile.TemporaryDirectory(prefix="loopflow-release-") as temp:
        directory = Path(temp)
        tag = _latest_release_tag()
        pinned_base = f"{RELEASE_DOWNLOAD_BASE}/{tag}"
        manifest = directory / "SHA256SUMS"
        _download_release_asset(f"{pinned_base}/SHA256SUMS", manifest)
        installer = directory / "install.sh"
        _download_release_asset(f"{pinned_base}/install.sh", installer)
        _verify_release_asset(installer, _manifest_digest(manifest, "install.sh"))
        env = {**os.environ, "LF_INSTALL_DIR": str(install_dir)}
        _run_or_raise(
            ["sh", str(installer), "--version", tag],
            "published release",
            env=env,
        )
        return tag


# --- Loopflow bundle ---


def _install_loopflow(spec: BundleSpec, version: str) -> None:
    if spec.app_path.exists():
        shutil.rmtree(spec.app_path)

    spec.macos_dir.mkdir(parents=True)
    spec.resources_dir.mkdir(parents=True)

    for executable in spec.executables:
        _atomic_install(executable, spec.macos_dir / _bundle_binary_name(executable))

    installed_plist = spec.contents_dir / spec.info_plist.name
    shutil.copy(spec.info_plist, installed_plist)
    stamp_bundle_version(installed_plist, version)
    for resource in spec.resources:
        shutil.copy(resource, spec.resources_dir / resource.name)
    (spec.contents_dir / "PkgInfo").write_text("APPL????")

    _verify_bundle_layout(spec)

    identity = _codesign_identity()
    if identity != "-":
        typer.echo(f"  [codesign] signing with stable identity: {identity}")
    sign = subprocess.run(
        [
            "codesign",
            "--force",
            "--deep",
            "--preserve-metadata=entitlements",
            "--sign",
            identity,
            str(spec.app_path),
        ],
        capture_output=True,
        text=True,
    )
    if sign.returncode != 0:
        detail = (sign.stderr or sign.stdout).strip()
        raise StageError(f"codesign failed: {detail}")

    _verify_bundle_signature(spec)


def _codesign_identity() -> str:
    """Pick a stable signing identity so keychain grants survive rebuilds.

    Ad-hoc signatures (`-`) change every build, which invalidates the macOS
    keychain ACL on items like `loopflow.connection.token` and re-prompts the
    user on every launch. A stable identity keeps "Always Allow" sticky. Falls
    back to ad-hoc when no identity exists (CI, fresh machines) so builds never
    break. Override with LOOPFLOW_CODESIGN_IDENTITY.
    """
    override = os.environ.get("LOOPFLOW_CODESIGN_IDENTITY")
    if override:
        return override
    found = subprocess.run(
        ["security", "find-identity", "-v", "-p", "codesigning"],
        capture_output=True,
        text=True,
    ).stdout
    for preferred in ("Developer ID Application", "Apple Development"):
        for line in found.splitlines():
            if preferred in line:
                start, end = line.find('"'), line.rfind('"')
                if start != -1 and end > start:
                    return line[start + 1 : end]
    return "-"


def _verify_bundle_layout(spec: BundleSpec) -> None:
    """Verify the app executable and bundled control helpers.

    Every declared executable must exist, be executable, and be a Mach-O binary
    that includes the current architecture. Catches missing files, wrong-arch
    copies, and non-executable placeholders before we hand the bundle to
    codesign.
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


# --- Commands ---


app = typer.Typer(help="Build and install loopflow locally.", add_completion=False)


@app.callback()
def _root() -> None:
    """Build and install loopflow locally."""


@app.command()
def refresh(
    install_dir: Path | None = typer.Option(
        None, "--install-dir", help="Install lf here instead of the resolved local bin dir"
    ),
) -> None:
    """Install the latest published release through the external-user path."""
    resolved_install_dir = install_dir.expanduser() if install_dir else _resolve_install_dir()

    try:
        tag = _install_published_release(resolved_install_dir)
    except StageError as exc:
        typer.echo(f"refresh failed: {exc}", err=True)
        raise typer.Exit(code=1) from exc

    target = resolved_install_dir / "lf"
    typer.echo(f"release: {tag}")
    typer.echo(f"installed: {target}")
    result = subprocess.run([str(target), "--version"], text=True)
    if result.returncode != 0:
        raise typer.Exit(code=result.returncode)


@app.command()
def local(
    dry_run: bool = typer.Option(False, "-n", "--dry-run", help="Show what would be done"),
    skip: list[str] = typer.Option(
        [], "--skip", help=f"Skip a build stage ({'|'.join(BUILD_STAGES)}); repeatable"
    ),
) -> None:
    """Build this worktree into local-bin/ with validation-only authority."""
    skip_set = set(skip)
    unknown = skip_set - set(BUILD_STAGES)
    if unknown:
        raise typer.BadParameter(f"unknown --skip values: {', '.join(sorted(unknown))}")

    spec = default_bundle_spec()
    version = read_release_version(ROOT)

    if dry_run:
        planned = [s for s in BUILD_STAGES if s not in skip_set] or ["(nothing)"]
        typer.echo(f"Would build in parallel: {', '.join(planned)}")
        typer.echo(f"Would stage lf + {spec.app_path.name} (v{version}) into {LOCAL_BIN}")
        executable_names = ", ".join(path.name for path in spec.executables)
        typer.echo(f"  Contents/MacOS/: {executable_names}")
        typer.echo("Would keep the validation-only build under local-bin/")
        return

    total_start = time.monotonic()
    try:
        _run_parallel_builds(skip_set)

        typer.echo(f"Staging lf + {APP_NAME}.app (v{version}) into {LOCAL_BIN}...")
        _stage_binaries(LOCAL_BIN)
        _install_loopflow(spec, version)
        typer.echo(f"Built {spec.app_path}")
        typer.echo(f"\nBuilt into {LOCAL_BIN}. Development builds cannot become production.")
    except StageError as exc:
        typer.echo(f"install failed: {exc}", err=True)
        raise typer.Exit(code=1) from exc

    elapsed = time.monotonic() - total_start
    typer.echo(f"\nTotal time: {elapsed:.1f}s")


if __name__ == "__main__":
    app()
