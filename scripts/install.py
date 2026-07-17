#!/usr/bin/env python3
"""Build and install loopflow locally.

    install.py local            # build this worktree into local-bin/
    install.py local --use      # promote this worktree onto PATH and /Applications
    install.py local --use --no-screens  # promote without refreshing website captures
    install.py local --skip swift
    install.py local -n         # dry run
    install.py refresh          # pull default branch, rebuild lf, install to PATH

Remote releases happen via `lf release patch` -> merge -> auto-tag -> CI.
"""

from __future__ import annotations

import os
import platform
import plistlib
import shutil
import subprocess
import sys
import threading
import time
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

import typer
from bundle_version import read_release_version, stamp_bundle_version

ROOT = Path(__file__).parent.parent
DEFAULT_INSTALL_DIR = Path.home() / ".local" / "bin"
APPLICATIONS_DIR = Path("/Applications")
LOCAL_BIN = ROOT / "local-bin"
APP_NAME = "Loopflow"
# Pre-rename bundle name, removed on upgrade so /Applications keeps one copy.
LEGACY_APP_NAME = "Concerto"
# SwiftPM executable product; the library owns the bare `Loopflow` name, so the
# app binary is built as `LoopflowMac` and renamed to APP_NAME inside the bundle.
SWIFT_APP_PRODUCT = "LoopflowMac"
BUILD_STAGES = ("cargo", "swift")


# --- Bundle spec (single source of truth for Loopflow.app layout) ---


@dataclass(frozen=True)
class BundleSpec:
    """Paths that define the Loopflow.app bundle."""

    repo_root: Path
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
        repo_root=root,
        app_path=root / "local-bin" / f"{APP_NAME}.app",
        executables=(
            swift / ".build" / "release" / "LoopflowMac",
            cargo_release / "lf",
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


# --- Git refresh ---


def _git_stdout(args: list[str], repo: Path = ROOT, check: bool = True) -> str:
    result = subprocess.run(
        ["git", "-C", str(repo), *args],
        capture_output=True,
        text=True,
        check=check,
    )
    if check:
        result.check_returncode()
    if result.returncode != 0:
        return ""
    return result.stdout.strip()


def _refresh_default_branch(repo: Path = ROOT) -> None:
    default_branch = _git_stdout(
        ["symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"],
        repo=repo,
        check=False,
    ).removeprefix("origin/")
    current_branch = _git_stdout(["branch", "--show-current"], repo=repo)
    if default_branch and current_branch != default_branch:
        raise StageError(
            f"refusing to pull {current_branch}; checkout {default_branch} or pass --no-pull"
        )

    typer.echo(f"Pulling {repo}...")
    if default_branch:
        _run_or_raise(["git", "fetch", "origin", default_branch], "git", cwd=repo)
        _run_or_raise(["git", "merge", "--ff-only", f"origin/{default_branch}"], "git", cwd=repo)
    else:
        _run_or_raise(["git", "fetch"], "git", cwd=repo)
        _run_or_raise(["git", "merge", "--ff-only", "@{upstream}"], "git", cwd=repo)


# --- Builds ---


def _migration_authority(repo: Path = ROOT) -> str:
    """Only a canonical main or tagged checkout may advance the release store."""
    head = _git_stdout(["rev-parse", "HEAD"], repo=repo, check=False)
    if not head:
        return "published"
    default_ref = _git_stdout(
        ["symbolic-ref", "--quiet", "refs/remotes/origin/HEAD"],
        repo=repo,
        check=False,
    )
    canonical = (
        _git_stdout(["rev-parse", default_ref], repo=repo, check=False) if default_ref else ""
    )
    tagged = bool(_git_stdout(["tag", "--points-at", "HEAD"], repo=repo, check=False))
    dirty = bool(_git_stdout(["status", "--porcelain"], repo=repo, check=False))
    return "published" if not dirty and (head == canonical or tagged) else "validation_only"


def _release_build_env() -> dict[str, str]:
    return {
        **os.environ,
        "LOOPFLOW_BUILD_PROVENANCE": "release",
        "LOOPFLOW_MIGRATION_AUTHORITY": os.environ.get(
            "LOOPFLOW_MIGRATION_AUTHORITY", _migration_authority()
        ),
    }


def _build_binaries() -> None:
    typer.echo("Building lf (cargo release)...")
    _run_or_raise(
        ["cargo", "build", "-p", "loopflow", "--release"],
        "cargo",
        cwd=ROOT,
        env=_release_build_env(),
    )


def _build_cli_binaries() -> None:
    typer.echo("Building lf (cargo release)...")
    _run_or_raise(
        ["cargo", "build", "--release", "-p", "loopflow", "--bin", "lf"],
        "cargo",
        cwd=ROOT,
        env=_release_build_env(),
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
    """Copy freshly built lf into this worktree's local-bin/."""
    _install_cli_binaries(local_bin)


def _install_cli_binaries(install_dir: Path) -> None:
    """Atomically copy freshly built lf into install_dir."""
    install_dir.mkdir(parents=True, exist_ok=True)
    built = ROOT / "target" / "release"
    _atomic_install(built / "lf", install_dir / "lf")


def _sync_skills(lf_bin: Path) -> None:
    """Mirror loopflow skills into ~/.claude/skills and ~/.agents/skills (codex).

    Runs after the binary install so the freshly built `lf` regenerates the
    global skill catalog. A failure here doesn't fail the install—the binaries
    are already in place—so we warn and move on.
    """
    typer.echo("Syncing skills into ~/.claude and ~/.agents...")
    try:
        code = _stream_process([str(lf_bin), "sync-skills", "--yes"], "skills", cwd=ROOT)
    except OSError as exc:
        typer.echo(f"skill sync failed ({exc}); binaries installed, skills unchanged", err=True)
        return
    if code != 0:
        typer.echo(
            f"skill sync failed (exit {code}); binaries installed, skills unchanged",
            err=True,
        )


def _promote(local_bin: Path, install_dir: Path, applications_dir: Path = APPLICATIONS_DIR) -> None:
    """Make this worktree's build the active one.

    Symlinks lf from the resolved bin dir to local-bin/ (so rebuilds take
    effect with no extra skill) and copies Loopflow.app into /Applications.
    """
    install_dir.mkdir(parents=True, exist_ok=True)
    link = install_dir / "lf"
    if link.is_symlink() or link.exists():
        link.unlink()
    link.symlink_to(local_bin / "lf")

    app_dst = applications_dir / f"{APP_NAME}.app"
    if app_dst.exists():
        shutil.rmtree(app_dst)
    # Drop the pre-rename bundle so /Applications doesn't keep two copies.
    legacy = applications_dir / f"{LEGACY_APP_NAME}.app"
    if legacy.exists():
        shutil.rmtree(legacy)
    shutil.copytree(local_bin / f"{APP_NAME}.app", app_dst, symlinks=True)


def _refresh_website_screens(lf_binary: Path) -> None:
    """Run the post-promote capture hook without making install depend on it."""
    command = [
        sys.executable,
        str(ROOT / "scripts/refresh_website_screens.py"),
        "--executable",
        str(APPLICATIONS_DIR / f"{APP_NAME}.app/Contents/MacOS/{APP_NAME}"),
        "--lf-binary",
        str(lf_binary),
        "--publish",
        "--skip-unavailable",
    ]
    result = subprocess.run(command, cwd=ROOT)
    if result.returncode != 0:
        typer.echo(
            "website-screens: refresh failed; the promoted build is still installed",
            err=True,
        )


# --- Loopflow bundle ---


def _source_provenance(repo_root: Path) -> tuple[str, bool]:
    commit = _git_stdout(["rev-parse", "HEAD"], repo=repo_root, check=False)
    if not commit:
        raise StageError(f"cannot identify source commit in {repo_root}")
    dirty = bool(_git_stdout(["status", "--porcelain"], repo=repo_root, check=False))
    return commit, dirty


def _stamp_source_provenance(info_plist: Path, commit: str, dirty: bool) -> None:
    data = plistlib.loads(info_plist.read_bytes())
    data["LoopflowSourceCommit"] = commit
    data["LoopflowSourceDirty"] = dirty
    info_plist.write_bytes(plistlib.dumps(data))


def _install_loopflow(spec: BundleSpec, version: str) -> None:
    source_commit, source_dirty = _source_provenance(spec.repo_root)
    if spec.app_path.exists():
        shutil.rmtree(spec.app_path)

    spec.macos_dir.mkdir(parents=True)
    spec.resources_dir.mkdir(parents=True)

    for executable in spec.executables:
        _atomic_install(executable, spec.macos_dir / _bundle_binary_name(executable))

    installed_plist = spec.contents_dir / spec.info_plist.name
    shutil.copy(spec.info_plist, installed_plist)
    stamp_bundle_version(installed_plist, version)
    _stamp_source_provenance(installed_plist, source_commit, source_dirty)
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
    """Verify the app executable and bundled `lf` helper.

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


# --- Reporting ---


def _report_path_collisions(install_dir: Path) -> None:
    found = shutil.which("lf")
    if not found:
        return
    resolved = Path(found)
    expected = install_dir / "lf"
    typer.echo(f"lf: {resolved}")
    if resolved != expected:
        typer.echo(
            f"Note: lf resolves to {resolved}, not {expected}. "
            f"Add {install_dir} to PATH to use the freshly installed binary.",
            err=True,
        )


# --- Commands ---


app = typer.Typer(help="Build and install loopflow locally.", add_completion=False)


@app.callback()
def _root() -> None:
    """Build and install loopflow locally."""


@app.command()
def refresh(
    no_pull: bool = typer.Option(
        False, "--no-pull", help="Build the current checkout without pulling"
    ),
    install_dir: Path | None = typer.Option(
        None, "--install-dir", help="Install lf here instead of the resolved local bin dir"
    ),
) -> None:
    """Pull the default branch, rebuild lf, and install it locally."""
    resolved_install_dir = install_dir.expanduser() if install_dir else _resolve_install_dir()

    try:
        if not no_pull:
            _refresh_default_branch(ROOT)
        _build_cli_binaries()
        _install_cli_binaries(resolved_install_dir)
        _sync_skills(resolved_install_dir / "lf")
    except (subprocess.CalledProcessError, StageError) as exc:
        typer.echo(f"refresh failed: {exc}", err=True)
        raise typer.Exit(code=1) from exc

    typer.echo(f"installed: {resolved_install_dir / 'lf'}")
    result = subprocess.run([str(resolved_install_dir / "lf"), "--version"], text=True)
    if result.returncode != 0:
        raise typer.Exit(code=result.returncode)


@app.command()
def local(
    use: bool = typer.Option(
        False, "--use", help="Promote this build: symlink lf onto PATH and install the app"
    ),
    dry_run: bool = typer.Option(False, "-n", "--dry-run", help="Show what would be done"),
    skip: list[str] = typer.Option(
        [], "--skip", help=f"Skip a build stage ({'|'.join(BUILD_STAGES)}); repeatable"
    ),
    no_screens: bool = typer.Option(
        False, "--no-screens", help="Do not refresh live website captures after promotion"
    ),
) -> None:
    """Build this worktree into local-bin/; --use makes it the active build."""
    skip_set = set(skip)
    unknown = skip_set - set(BUILD_STAGES)
    if unknown:
        raise typer.BadParameter(f"unknown --skip values: {', '.join(sorted(unknown))}")

    spec = default_bundle_spec()
    install_dir = _resolve_install_dir()
    version = read_release_version(ROOT)

    if dry_run:
        planned = [s for s in BUILD_STAGES if s not in skip_set] or ["(nothing)"]
        typer.echo(f"Would build in parallel: {', '.join(planned)}")
        typer.echo(f"Would stage lf + {spec.app_path.name} (v{version}) into {LOCAL_BIN}")
        executable_names = ", ".join(path.name for path in spec.executables)
        typer.echo(f"  Contents/MacOS/: {executable_names}")
        if use:
            typer.echo(f"Would promote: symlink lf into {install_dir}")
            typer.echo(f"Would install {APPLICATIONS_DIR / f'{APP_NAME}.app'}")
            typer.echo("Would sync skills into ~/.claude/skills and ~/.agents/skills")
            if not no_screens:
                typer.echo("Would refresh live website captures (skips if live state is absent)")
        else:
            typer.echo("Would not promote (pass --use to symlink onto PATH and install the app)")
        return

    total_start = time.monotonic()
    try:
        _run_parallel_builds(skip_set)

        typer.echo(f"Staging lf + {APP_NAME}.app (v{version}) into {LOCAL_BIN}...")
        _stage_binaries(LOCAL_BIN)
        _install_loopflow(spec, version)
        typer.echo(f"Built {spec.app_path}")

        if use:
            typer.echo(f"Promoting this worktree (symlinking lf into {install_dir})...")
            _promote(LOCAL_BIN, install_dir)
            typer.echo(f"Installed {APPLICATIONS_DIR / f'{APP_NAME}.app'}")
            _report_path_collisions(install_dir)
            _sync_skills(install_dir / "lf")
            if not no_screens:
                _refresh_website_screens(install_dir / "lf")

        else:
            typer.echo(f"\nBuilt into {LOCAL_BIN}. Run with --use to make it the active build.")
    except StageError as exc:
        typer.echo(f"install failed: {exc}", err=True)
        raise typer.Exit(code=1) from exc

    elapsed = time.monotonic() - total_start
    typer.echo(f"\nTotal time: {elapsed:.1f}s")


if __name__ == "__main__":
    app()
