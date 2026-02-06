#!/usr/bin/env python3
"""Publish loopflow to PyPI and Concerto DMG to R2.

publish.py                # publish current version (tag + push)
publish.py minor          # bump to next minor, then publish
publish.py local          # build and install locally
publish.py dmg            # upload existing DMG
publish.py screenshots    # generate screenshots
"""

import os
import re
import subprocess
import sys
from pathlib import Path

import typer

ROOT = Path(__file__).parent.parent
VERSION_FILE = ROOT / "VERSION"
R2_PUBLIC_URL = "https://downloads.loopflow.studio"

app = typer.Typer(
    help="Build and publish loopflow.",
    invoke_without_command=True,
)


# --- Version management ---


def _get_version() -> str:
    if VERSION_FILE.exists():
        return VERSION_FILE.read_text().strip()
    pyproject = ROOT / "pyproject.toml"
    content = pyproject.read_text()
    match = re.search(r'^version = "([^"]+)"', content, re.MULTILINE)
    if match:
        return match.group(1)
    raise RuntimeError("Could not find version")


def _check_versions() -> str:
    """Verify VERSION, Cargo.toml, and pyproject.toml all agree. Returns version or raises."""
    versions: dict[str, str] = {}

    if VERSION_FILE.exists():
        versions["VERSION"] = VERSION_FILE.read_text().strip()

    cargo = ROOT / "Cargo.toml"
    if cargo.exists():
        m = re.search(r'^version = "([^"]+)"', cargo.read_text(), re.MULTILINE)
        if m:
            versions["Cargo.toml"] = m.group(1)

    pyproject = ROOT / "pyproject.toml"
    if pyproject.exists():
        m = re.search(r'^version = "([^"]+)"', pyproject.read_text(), re.MULTILINE)
        if m:
            versions["pyproject.toml"] = m.group(1)

    unique = set(versions.values())
    if len(unique) != 1:
        lines = [f"  {f}: {v}" for f, v in versions.items()]
        typer.echo("Version mismatch:\n" + "\n".join(lines), err=True)
        raise typer.Exit(code=1)

    return list(unique)[0]


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


def _write_version(version: str) -> None:
    VERSION_FILE.write_text(version + "\n")

    pyproject = ROOT / "pyproject.toml"
    content = pyproject.read_text()
    content = re.sub(r'^version = "[^"]+"', f'version = "{version}"', content, flags=re.MULTILINE)
    pyproject.write_text(content)

    cargo_toml = ROOT / "Cargo.toml"
    content = cargo_toml.read_text()
    content = re.sub(r'^version = "[^"]+"', f'version = "{version}"', content, flags=re.MULTILINE)
    cargo_toml.write_text(content)


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
        [
            "uv", "run", "maturin", "build", "--release",
            "--manifest-path", str(ROOT / "rust" / "loopflow-py" / "Cargo.toml"),
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0, result.stdout + result.stderr


def _install_wheel() -> tuple[bool, str]:
    wheels = sorted((ROOT / "target" / "wheels").glob("loopflow-*.whl"))
    if not wheels:
        return False, "No wheel found in target/wheels/"
    result = subprocess.run(
        ["uv", "tool", "install", "--force", str(wheels[-1])],
        capture_output=True,
        text=True,
    )
    return result.returncode == 0, result.stdout + result.stderr


def _install_binaries() -> tuple[bool, str]:
    install_dir = os.environ.get("LF_INSTALL_DIR", str(Path.home() / ".local" / "bin"))
    install_path = Path(install_dir)
    install_path.mkdir(parents=True, exist_ok=True)

    result = subprocess.run(
        ["cargo", "build", "-p", "lf", "-p", "lfd", "--release"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return False, result.stdout + result.stderr

    built = ROOT / "target" / "release"
    for name in ("lf", "lfd"):
        src = built / name
        if src.exists():
            dst = install_path / name
            dst.write_bytes(src.read_bytes())
            dst.chmod(0o755)

    return True, f"Installed lf/lfd to {install_path}"


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


# --- Git helpers ---


def _tag_and_push(version: str) -> None:
    subprocess.run(["git", "tag", f"v{version}"], cwd=ROOT, check=True)
    typer.echo("Pushing to origin...")
    subprocess.run(["git", "push", "origin", "main"], cwd=ROOT, check=True)
    subprocess.run(["git", "push", "origin", f"v{version}"], cwd=ROOT, check=True)


# --- Release helpers ---


def _release(bump_type: str, dry_run: bool, skip_dmg: bool, skip_screenshots: bool) -> None:
    _check_on_main()
    version = _check_versions()
    new_version = _bump_version(version, bump_type)

    if dry_run:
        if not skip_screenshots:
            typer.echo("Would generate screenshots")
        if not skip_dmg:
            typer.echo("Would build and upload DMG")
        typer.echo(f"Would bump version: {version} → {new_version}")
        typer.echo(f"Would commit and tag v{new_version}")
        typer.echo("Would push tag to trigger CI release")
        return

    if not skip_screenshots:
        typer.echo("Generating screenshots...")
        ok, output = _generate_screenshots()
        if not ok:
            typer.echo(f"Screenshot generation failed (continuing):\n{output}", err=True)
        else:
            typer.echo("Screenshots generated.")

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

    typer.echo(f"Bumping version: {version} → {new_version}")
    _write_version(new_version)
    subprocess.run(["git", "add", "VERSION", "pyproject.toml", "Cargo.toml", "Cargo.lock"], cwd=ROOT, check=True)
    subprocess.run(["git", "commit", "-m", f"release: v{new_version}"], cwd=ROOT, check=True)

    _tag_and_push(new_version)
    typer.echo(f"\nPublished v{new_version}")
    typer.echo("CI will build and publish to PyPI.")


# --- Commands ---


@app.callback(invoke_without_command=True)
def publish(
    ctx: typer.Context,
    dry_run: bool = typer.Option(False, "-n", "--dry-run", help="Show what would be done"),
):
    """Publish current version, or use a subcommand (patch/minor/major/local/dmg/screenshots)."""
    if ctx.invoked_subcommand is not None:
        return

    _check_on_main()
    version = _check_versions()

    if dry_run:
        typer.echo(f"Would tag and push v{version}")
        return

    _tag_and_push(version)
    typer.echo(f"\nPublished v{version}")
    typer.echo("CI will build and publish to PyPI.")


@app.command()
def patch(
    dry_run: bool = typer.Option(False, "-n", "--dry-run", help="Show what would be done"),
    skip_dmg: bool = typer.Option(False, "--skip-dmg", help="Skip DMG build/upload"),
    skip_screenshots: bool = typer.Option(False, "--skip-screenshots", help="Skip screenshot generation"),
):
    """Bump patch version and publish."""
    _release("patch", dry_run, skip_dmg, skip_screenshots)


@app.command()
def minor(
    dry_run: bool = typer.Option(False, "-n", "--dry-run", help="Show what would be done"),
    skip_dmg: bool = typer.Option(False, "--skip-dmg", help="Skip DMG build/upload"),
    skip_screenshots: bool = typer.Option(False, "--skip-screenshots", help="Skip screenshot generation"),
):
    """Bump minor version and publish."""
    _release("minor", dry_run, skip_dmg, skip_screenshots)


@app.command()
def major(
    dry_run: bool = typer.Option(False, "-n", "--dry-run", help="Show what would be done"),
    skip_dmg: bool = typer.Option(False, "--skip-dmg", help="Skip DMG build/upload"),
    skip_screenshots: bool = typer.Option(False, "--skip-screenshots", help="Skip screenshot generation"),
):
    """Bump major version and publish."""
    _release("major", dry_run, skip_dmg, skip_screenshots)


@app.command()
def local(
    dry_run: bool = typer.Option(False, "-n", "--dry-run", help="Show what would be done"),
):
    """Build and install locally (no publish)."""
    if dry_run:
        typer.echo("Would build wheel with maturin")
        typer.echo("Would install with uv tool install")
        typer.echo("Would install lf/lfd binaries")
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
    ok, output = _install_binaries()
    if not ok:
        typer.echo(f"Binary install failed:\n{output}", err=True)
        raise typer.Exit(code=1)
    typer.echo(output)

    for name in ("lf", "lfd"):
        result = subprocess.run(["which", name], capture_output=True, text=True)
        if result.returncode == 0:
            typer.echo(f"{name}: {result.stdout.strip()}")


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
