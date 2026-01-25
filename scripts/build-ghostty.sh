#!/bin/bash
# Build Ghostty xcframework for Concerto embedded terminal
#
# Prerequisites:
#   - Zig 0.15.x: brew install zig
#   - Metal toolchain: xcodebuild -downloadComponent MetalToolchain
#
# Usage:
#   ./scripts/build-ghostty.sh
#
# Output:
#   vendor/ghostty/macos/GhosttyKit.xcframework

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
GHOSTTY_DIR="$PROJECT_ROOT/vendor/ghostty"

# Check prerequisites
check_zig() {
    if ! command -v zig &> /dev/null; then
        echo "Error: Zig not found. Install with: brew install zig"
        exit 1
    fi

    ZIG_VERSION=$(zig version)
    echo "Using Zig $ZIG_VERSION"

    # Check for 0.15.x
    if [[ ! "$ZIG_VERSION" =~ ^0\.15\. ]]; then
        echo "Warning: Ghostty requires Zig 0.15.x, found $ZIG_VERSION"
    fi
}

check_metal_toolchain() {
    if ! xcrun -sdk macosx metal --version &> /dev/null; then
        echo "Metal toolchain not found. Downloading..."
        xcodebuild -downloadComponent MetalToolchain
    fi
}

build_ghostty() {
    echo "Building Ghostty..."
    cd "$GHOSTTY_DIR"
    zig build -Doptimize=ReleaseFast

    if [[ -d "macos/GhosttyKit.xcframework" ]]; then
        echo "Success: GhosttyKit.xcframework built"
        ls -la macos/GhosttyKit.xcframework/
    else
        echo "Error: GhosttyKit.xcframework not found"
        exit 1
    fi
}

main() {
    echo "=== Building Ghostty for Concerto ==="
    check_zig
    check_metal_toolchain
    build_ghostty
    echo "=== Done ==="
}

main "$@"
