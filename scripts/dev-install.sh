#!/bin/bash
# Build and install lf binary for local development.
# Bypasses maturin/uv wheel caching that can serve stale binaries.
set -e

cargo build -p lf
cp target/debug/lf .venv/bin/lf
cp target/debug/lfd .venv/bin/lfd 2>/dev/null || true
echo "Installed $(target/debug/lf --version) to .venv/bin/"
