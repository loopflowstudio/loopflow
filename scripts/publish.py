#!/usr/bin/env python3
"""Publish loopflow. Version comes from git tags.

publish.py                # recover: re-push tag, rerun CI if needed
publish.py patch          # bump patch, tag, push tag → CI releases
publish.py local          # build and install locally
publish.py dmg            # upload existing DMG
publish.py screenshots    # generate screenshots
"""

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import Request, urlopen

import typer

ROOT = Path(__file__).parent.parent
R2_PUBLIC_URL = "https://downloads.loopflow.studio"
DEFAULT_INSTALL_DIR = Path.home() / ".local" / "bin"

app = typer.Typer(
    help="Build and publish loopflow.",
    invoke_without_command=True,
)


# --- Version management ---


def _get_version() -> str:
    result = subprocess.run(
        ["git", "describe", "--tags", "--abbrev=0"],
        cwd=ROOT, capture_output=True, text=True,
    )
    if result.returncode != 0:
        raise RuntimeError("No version tags found")
    return result.stdout.strip().lstrip("v")


def _bump_version(version: str, bump_type: str) -> str:
    parts = version.split(".")
    if len(parts) != 3:
        raise ValueError(f"Invalid version format: {version}")
    major, minor, patch = map(int, parts)

    if bump_type == "major":
        return f"{major + 1}.0.0"
    elif bump_type == "minor":
        return f"{major}.{minor + 1}.0"
    else:
        return f"{major}.{minor}.{patch + 1}"


# --- Preconditions ---


def _check_on_main() -> None:
    result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    branch = result.stdout.strip()
    if branch != "main":
        typer.echo(f"Error: Not on main branch (current: {branch})", err=True)
        raise typer.Exit(code=1)

    subprocess.run(["git", "fetch", "origin", "main"], cwd=ROOT, capture_output=True)
    result = subprocess.run(
        ["git", "rev-parse", "HEAD", "origin/main"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    shas = result.stdout.strip().split("\n")
    if len(shas) == 2 and shas[0] != shas[1]:
        typer.echo("Error: Local main differs from origin/main. Pull or push first.", err=True)
        raise typer.Exit(code=1)

    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if result.stdout.strip():
        typer.echo("Error: Uncommitted changes in working directory", err=True)
        raise typer.Exit(code=1)


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


def _build_dmg() -> tuple[bool, str]:
    swift_dir = ROOT / "swift"
    if not swift_dir.exists():
        return False, f"swift directory not found: {swift_dir}"
    result = subprocess.run(
        [sys.executable, "scripts/dev.py", "release"],
        cwd=swift_dir,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0, result.stdout + result.stderr


def _upload_dmg(version: str) -> tuple[bool, str]:
    try:
        import boto3
    except ImportError:
        return False, "boto3 required for DMG upload: pip install boto3"

    account_id = os.environ.get("R2_ACCOUNT_ID")
    access_key = os.environ.get("R2_ACCESS_KEY_ID")
    secret_key = os.environ.get("R2_SECRET_ACCESS_KEY")
    bucket = os.environ.get("R2_BUCKET_NAME", "loopflow-downloads")

    if not all([account_id, access_key, secret_key]):
        return False, (
            "R2 credentials not set. Required environment variables:\n"
            "  R2_ACCOUNT_ID\n"
            "  R2_ACCESS_KEY_ID\n"
            "  R2_SECRET_ACCESS_KEY"
        )

    dmg_path = ROOT / "swift" / "dist" / "LoopflowConcerto.dmg"
    if not dmg_path.exists():
        return False, f"DMG not found: {dmg_path}"

    client = boto3.client(
        "s3",
        endpoint_url=f"https://{account_id}.r2.cloudflarestorage.com",
        aws_access_key_id=access_key,
        aws_secret_access_key=secret_key,
    )

    versioned_key = f"LoopflowConcerto-{version}.dmg"
    latest_key = "LoopflowConcerto-latest.dmg"

    try:
        client.upload_file(
            str(dmg_path), bucket, versioned_key,
            ExtraArgs={"ContentType": "application/x-apple-diskimage", "CacheControl": "public, max-age=31536000, immutable"},
        )
        client.upload_file(
            str(dmg_path), bucket, latest_key,
            ExtraArgs={"ContentType": "application/x-apple-diskimage", "CacheControl": "public, max-age=60"},
        )
        return True, f"Uploaded to {R2_PUBLIC_URL}/{versioned_key}"
    except Exception as e:
        return False, f"Upload failed: {e}"


def _generate_screenshots() -> tuple[bool, str]:
    script = ROOT / "scripts" / "generate_screenshots.py"
    result = subprocess.run(
        [sys.executable, str(script)],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0, result.stdout + result.stderr


# --- Registry checks ---


def _pypi_has_version(version: str) -> bool:
    try:
        urlopen(Request(f"https://pypi.org/pypi/loopflow/{version}/json", method="HEAD"))
        return True
    except HTTPError:
        return False


CRATE_NAMES = ["loopflow"]


def _crates_has_version(version: str) -> bool:
    for crate in CRATE_NAMES:
        try:
            urlopen(Request(f"https://crates.io/api/v1/crates/{crate}/{version}", method="HEAD"))
            return True
        except HTTPError:
            continue
    return False


def _registries_have_version(version: str) -> list[str]:
    """Return list of registries that already have this version."""
    found = []
    if _pypi_has_version(version):
        found.append("PyPI")
    if _crates_has_version(version):
        found.append("crates.io")
    return found


# --- Git helpers ---


def _head_sha() -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True,
    )
    return result.stdout.strip()


def _local_tag_sha(tag: str) -> str | None:
    result = subprocess.run(
        ["git", "rev-parse", f"refs/tags/{tag}"], cwd=ROOT, capture_output=True, text=True,
    )
    if result.returncode != 0:
        return None
    return result.stdout.strip()


def _remote_tag_sha(tag: str) -> str | None:
    result = subprocess.run(
        ["git", "ls-remote", "origin", f"refs/tags/{tag}"], cwd=ROOT, capture_output=True, text=True,
    )
    line = result.stdout.strip()
    if not line:
        return None
    return line.split()[0]


def _release_workflow_status(version: str) -> str:
    """Check the release workflow status for a tag. Returns 'success', 'failure', 'in_progress', or 'none'."""
    result = subprocess.run(
        ["gh", "run", "list", "--workflow=release.yml", "--limit=10", "--json=headBranch,conclusion,status"],
        cwd=ROOT, capture_output=True, text=True,
    )
    if result.returncode != 0:
        return "none"
    runs = json.loads(result.stdout)
    tag = f"v{version}"
    for run in runs:
        if run.get("headBranch") == tag:
            if run.get("status") in ("in_progress", "queued", "waiting"):
                return "in_progress"
            return run.get("conclusion", "none")
    return "none"


def _rerun_release(version: str) -> bool:
    result = subprocess.run(
        ["gh", "run", "list", "--workflow=release.yml", "--limit=10", "--json=headBranch,databaseId,conclusion"],
        cwd=ROOT, capture_output=True, text=True,
    )
    if result.returncode != 0:
        return False
    runs = json.loads(result.stdout)
    tag = f"v{version}"
    for run in runs:
        if run.get("headBranch") == tag and run.get("conclusion") == "failure":
            run_id = run["databaseId"]
            typer.echo(f"Re-running failed release workflow (run {run_id})...")
            result = subprocess.run(
                ["gh", "run", "rerun", str(run_id), "--failed"],
                cwd=ROOT, capture_output=True, text=True,
            )
            return result.returncode == 0
    return False


def _ensure_released(version: str) -> None:
    """Ensure version is tagged, pushed, and CI release is running. Idempotent."""
    tag = f"v{version}"
    head = _head_sha()

    # Step 1: Ensure local tag exists at HEAD
    local_sha = _local_tag_sha(tag)
    if local_sha is None:
        typer.echo(f"Creating tag {tag}...")
        subprocess.run(["git", "tag", tag], cwd=ROOT, check=True)
    elif local_sha != head:
        registries = _registries_have_version(version)
        if registries:
            typer.echo(
                f"Error: {tag} points to a different commit, and {', '.join(registries)} already has v{version}.\n"
                f"Use `publish.py patch` to release a new version.",
                err=True,
            )
            raise typer.Exit(code=1)
        typer.echo(f"Moving tag {tag} to HEAD (previous release never completed)...")
        subprocess.run(["git", "tag", "-f", tag], cwd=ROOT, check=True)
    else:
        typer.echo(f"Tag {tag} already exists at HEAD.")

    # Step 2: Ensure tag is on remote at HEAD
    remote_sha = _remote_tag_sha(tag)
    if remote_sha is None:
        typer.echo(f"Pushing tag {tag}...")
        subprocess.run(["git", "push", "origin", tag], cwd=ROOT, check=True)
    elif remote_sha != head:
        registries = _registries_have_version(version)
        if registries:
            typer.echo(
                f"Error: Remote {tag} points to a different commit, and {', '.join(registries)} already has v{version}.\n"
                f"Use `publish.py patch` to release a new version.",
                err=True,
            )
            raise typer.Exit(code=1)
        typer.echo(f"Force-pushing tag {tag} to HEAD...")
        subprocess.run(["git", "push", "--force", "origin", tag], cwd=ROOT, check=True)
    else:
        typer.echo(f"Tag {tag} already on remote.")

    # Step 4: Check CI release status
    status = _release_workflow_status(version)
    if status == "success":
        typer.echo(f"v{version} already released successfully.")
        return
    elif status == "in_progress":
        typer.echo(f"Release workflow for v{version} is in progress.")
        return
    elif status == "failure":
        if _rerun_release(version):
            typer.echo("Release workflow re-triggered.")
        else:
            typer.echo("Could not re-trigger release. Re-run manually: gh run rerun <id> --failed", err=True)
            raise typer.Exit(code=1)
    else:
        # Tag push should trigger CI. If we just pushed it, CI will pick it up.
        # If tag was already on remote and no workflow found, force-push may have re-triggered.
        typer.echo("Tag pushed. CI release workflow should start shortly.")


# --- Release helpers ---


def _release(bump_type: str, dry_run: bool, skip_dmg: bool) -> None:
    _check_on_main()
    version = _get_version()
    new_version = _bump_version(version, bump_type)

    if dry_run:
        if not skip_dmg:
            typer.echo("Would build and upload DMG")
        typer.echo(f"Would tag v{new_version} (current: v{version})")
        typer.echo("Would push tag to trigger CI release")
        return

    if not skip_dmg:
        typer.echo("Building Concerto DMG...")
        ok, output = _build_dmg()
        if not ok:
            typer.echo(f"DMG build failed (continuing):\n{output}", err=True)
        else:
            typer.echo("DMG built.")
            typer.echo("Uploading DMG...")
            ok, output = _upload_dmg(new_version)
            if not ok:
                typer.echo(f"DMG upload failed (continuing):\n{output}", err=True)
            else:
                typer.echo(output)

    typer.echo(f"Tagging v{new_version} (was v{version})")
    _ensure_released(new_version)
    typer.echo(f"\nPublished v{new_version}")
    typer.echo("CI will build and publish.")


# --- Service management ---


def _restart_lfd() -> None:
    label = "com.loopflow.lfd"
    plist = Path.home() / "Library" / "LaunchAgents" / f"{label}.plist"

    # Use `lfd install` to regenerate the plist (captures current PATH)
    # and reload the service in one step.
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


@app.callback(invoke_without_command=True)
def publish(
    ctx: typer.Context,
    dry_run: bool = typer.Option(False, "-n", "--dry-run", help="Show what would be done"),
):
    """Recover latest release: re-push tag, rerun CI if needed."""
    if ctx.invoked_subcommand is not None:
        return

    _check_on_main()
    version = _get_version()

    if dry_run:
        typer.echo(f"Would ensure v{version} is released (re-push tag, check CI)")
        return

    _ensure_released(version)
    typer.echo(f"\nv{version} release ensured.")
    typer.echo("CI will build and publish.")


@app.command()
def patch(
    dry_run: bool = typer.Option(False, "-n", "--dry-run", help="Show what would be done"),
    skip_dmg: bool = typer.Option(False, "--skip-dmg", help="Skip DMG build/upload"),
):
    """Bump patch version and publish."""
    _release("patch", dry_run, skip_dmg)


@app.command()
def minor(
    dry_run: bool = typer.Option(False, "-n", "--dry-run", help="Show what would be done"),
    skip_dmg: bool = typer.Option(False, "--skip-dmg", help="Skip DMG build/upload"),
):
    """Bump minor version and publish."""
    _release("minor", dry_run, skip_dmg)


@app.command()
def major(
    dry_run: bool = typer.Option(False, "-n", "--dry-run", help="Show what would be done"),
    skip_dmg: bool = typer.Option(False, "--skip-dmg", help="Skip DMG build/upload"),
):
    """Bump major version and publish."""
    _release("major", dry_run, skip_dmg)


@app.command()
def local(
    dry_run: bool = typer.Option(False, "-n", "--dry-run", help="Show what would be done"),
):
    """Build and install locally (no publish)."""
    if dry_run:
        typer.echo("Would build wheel with maturin")
        typer.echo("Would install wheel with uv pip install")
        typer.echo("Would install lf/lfd binaries")
        typer.echo("Would install Concerto to /Applications")
        typer.echo("Would restart lfd")
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

    _restart_lfd()


@app.command()
def dmg(
    dry_run: bool = typer.Option(False, "-n", "--dry-run", help="Show what would be done"),
):
    """Upload existing Concerto DMG to R2."""
    version = _get_version()
    if dry_run:
        typer.echo(f"Would upload DMG to {R2_PUBLIC_URL}/LoopflowConcerto-{version}.dmg")
        return
    typer.echo("Uploading DMG...")
    ok, output = _upload_dmg(version)
    if not ok:
        typer.echo(f"DMG upload failed:\n{output}", err=True)
        raise typer.Exit(code=1)
    typer.echo(output)


@app.command()
def screenshots(
    dry_run: bool = typer.Option(False, "-n", "--dry-run", help="Show what would be done"),
):
    """Generate app screenshots."""
    if dry_run:
        typer.echo("Would generate screenshots")
        return
    typer.echo("Generating screenshots...")
    ok, output = _generate_screenshots()
    if not ok:
        typer.echo(f"Screenshot generation failed:\n{output}", err=True)
        raise typer.Exit(code=1)
    typer.echo("Screenshots generated.")


if __name__ == "__main__":
    app()
