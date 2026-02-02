# 04: Distribution

Distribute lf and lfd as single binaries via multiple channels.

## Context

Today:
- Python `lf` installed via `uv tool install loopflow`
- Rust `lf-engine` built from source
- Rust `lfd` built from source

Target: Single-binary distribution like ruff, uv, ripgrep.

## Goal

1. `brew install loopflow` on macOS
2. `cargo install loopflow` anywhere
3. `curl -fsSL https://loopflow.studio/install.sh | sh` anywhere
4. PyPI package bundles Rust binaries (uv-style)

## Binaries

| Binary | Purpose |
|--------|---------|
| `lf` | CLI for running steps/flows |
| `lfd` | Daemon for waves |

Both in single `loopflow` package. Or separate: `loopflow` (lf) and `loopflow-daemon` (lfd).

## Platforms

| Platform | Architecture |
|----------|--------------|
| macOS | arm64 (Apple Silicon) |
| macOS | x86_64 (Intel) |
| Linux | x86_64 |
| Linux | aarch64 |
| Windows | x86_64 (stretch goal) |

## Implementation

### Cargo Workspace

```toml
# Cargo.toml (workspace root)
[workspace]
members = [
    "rust/loopflow-engine",
    "rust/lf",
    "rust/lfd",
]

[workspace.package]
version = "0.8.0"
edition = "2021"
license = "MIT"
repository = "https://github.com/loopflowstudio/loopflow"
```

```toml
# rust/lf/Cargo.toml
[package]
name = "lf"
version.workspace = true
edition.workspace = true

[[bin]]
name = "lf"
path = "src/main.rs"

[dependencies]
loopflow-engine = { path = "../loopflow-engine" }
clap = { version = "4", features = ["derive"] }
anyhow = "1"
# ...
```

### GitHub Actions Release

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    strategy:
      matrix:
        include:
          - target: x86_64-apple-darwin
            os: macos-latest
          - target: aarch64-apple-darwin
            os: macos-latest
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
          - target: aarch64-unknown-linux-gnu
            os: ubuntu-latest

    runs-on: ${{ matrix.os }}

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Install cross (Linux aarch64)
        if: matrix.target == 'aarch64-unknown-linux-gnu'
        run: cargo install cross

      - name: Build
        run: |
          if [ "${{ matrix.target }}" = "aarch64-unknown-linux-gnu" ]; then
            cross build --release --target ${{ matrix.target }} -p lf -p lfd
          else
            cargo build --release --target ${{ matrix.target }} -p lf -p lfd
          fi

      - name: Package
        run: |
          mkdir -p dist
          cp target/${{ matrix.target }}/release/lf dist/
          cp target/${{ matrix.target }}/release/lfd dist/
          tar -czvf loopflow-${{ matrix.target }}.tar.gz -C dist .

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: loopflow-${{ matrix.target }}
          path: loopflow-${{ matrix.target }}.tar.gz

  release:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - name: Download artifacts
        uses: actions/download-artifact@v4

      - name: Create Release
        uses: softprops/action-gh-release@v1
        with:
          files: |
            loopflow-*/loopflow-*.tar.gz
          generate_release_notes: true
```

### Homebrew Formula

```ruby
# Formula/loopflow.rb
class Loopflow < Formula
  desc "Run steps and flows with coding agents"
  homepage "https://loopflow.studio"
  version "0.8.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/loopflowstudio/loopflow/releases/download/v#{version}/loopflow-aarch64-apple-darwin.tar.gz"
      sha256 "..."
    end
    on_intel do
      url "https://github.com/loopflowstudio/loopflow/releases/download/v#{version}/loopflow-x86_64-apple-darwin.tar.gz"
      sha256 "..."
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/loopflowstudio/loopflow/releases/download/v#{version}/loopflow-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "..."
    end
    on_intel do
      url "https://github.com/loopflowstudio/loopflow/releases/download/v#{version}/loopflow-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "..."
    end
  end

  def install
    bin.install "lf"
    bin.install "lfd"
  end

  service do
    run [opt_bin/"lfd", "run"]
    keep_alive true
    log_path var/"log/lfd.log"
    error_log_path var/"log/lfd.err"
  end

  test do
    system "#{bin}/lf", "--version"
    system "#{bin}/lfd", "--version"
  end
end
```

Tap setup:
```bash
# Users install via:
brew tap loopflowstudio/tap
brew install loopflow

# Or direct:
brew install loopflowstudio/tap/loopflow
```

### Install Script

```bash
#!/bin/bash
# install.sh - curl -fsSL https://loopflow.studio/install.sh | sh

set -e

REPO="loopflowstudio/loopflow"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Detect platform
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
  darwin) OS="apple-darwin" ;;
  linux) OS="unknown-linux-gnu" ;;
  *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
  x86_64) ARCH="x86_64" ;;
  arm64|aarch64) ARCH="aarch64" ;;
  *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

TARGET="${ARCH}-${OS}"

# Get latest version
VERSION=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')

if [ -z "$VERSION" ]; then
  echo "Failed to get latest version"
  exit 1
fi

echo "Installing loopflow v$VERSION for $TARGET..."

# Download
URL="https://github.com/$REPO/releases/download/v$VERSION/loopflow-$TARGET.tar.gz"
TMPDIR=$(mktemp -d)
curl -sL "$URL" | tar xz -C "$TMPDIR"

# Install
mkdir -p "$INSTALL_DIR"
mv "$TMPDIR/lf" "$INSTALL_DIR/"
mv "$TMPDIR/lfd" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/lf" "$INSTALL_DIR/lfd"

rm -rf "$TMPDIR"

echo "Installed to $INSTALL_DIR"
echo ""
echo "Add to PATH if needed:"
echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
echo ""
echo "Run:"
echo "  lf --help"
echo "  lfd install  # Install as service"
```

### Cargo Install

```toml
# rust/lf/Cargo.toml
[package]
name = "loopflow"  # crates.io name
version.workspace = true

[[bin]]
name = "lf"
path = "src/main.rs"
```

```bash
# Users install via:
cargo install loopflow

# Installs `lf` binary
```

For lfd separately:
```toml
# rust/lfd/Cargo.toml
[package]
name = "loopflow-daemon"
version.workspace = true

[[bin]]
name = "lfd"
path = "src/main.rs"
```

```bash
cargo install loopflow-daemon
```

### PyPI with Bundled Binaries

Like ruff and uv, bundle platform-specific Rust binaries in the Python wheel.

```toml
# pyproject.toml
[project]
name = "loopflow"
version = "0.8.0"

[project.scripts]
lf = "loopflow._bin:main"
lfd = "loopflow._bin:daemon"

[tool.maturin]
bindings = "bin"
strip = true
```

```python
# src/loopflow/_bin.py
import os
import sys
import subprocess

def _find_binary(name):
    """Find the bundled binary."""
    # Check in package directory
    pkg_dir = os.path.dirname(__file__)
    binary = os.path.join(pkg_dir, "bin", name)
    if os.path.exists(binary):
        return binary
    # Check in PATH
    return name

def main():
    binary = _find_binary("lf")
    sys.exit(subprocess.call([binary] + sys.argv[1:]))

def daemon():
    binary = _find_binary("lfd")
    sys.exit(subprocess.call([binary] + sys.argv[1:]))
```

Build with maturin:
```bash
maturin build --release
```

### Version Management

Single source of truth in workspace Cargo.toml:

```toml
[workspace.package]
version = "0.8.0"
```

Python version synced via:
```python
# src/loopflow/__init__.py
__version__ = "0.8.0"  # Updated by release script
```

Release script:
```bash
#!/bin/bash
VERSION=$1

# Update Cargo.toml
sed -i '' "s/^version = .*/version = \"$VERSION\"/" Cargo.toml

# Update Python
sed -i '' "s/__version__ = .*/__version__ = \"$VERSION\"/" src/loopflow/__init__.py

# Tag and push
git add -A
git commit -m "Release v$VERSION"
git tag "v$VERSION"
git push && git push --tags
```

## Done When

- [ ] `brew install loopflowstudio/tap/loopflow` works on macOS
- [ ] `cargo install loopflow` works
- [ ] `curl -fsSL https://loopflow.studio/install.sh | sh` works
- [ ] `uv tool install loopflow` installs Rust binaries
- [ ] Binaries built for all target platforms
- [ ] GitHub releases created automatically on tag
- [ ] Version numbers synchronized across Cargo/Python
- [ ] `lf --version` and `lfd --version` show correct version

## Dependencies

- Requires: 01-lf-cli, 02-lfd-primary, 03-service
- Enables: Users can actually install loopflow
