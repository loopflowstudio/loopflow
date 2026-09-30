#!/usr/bin/env bash
# Build Desktop and run tests with WindowServer connections denied, even on a
# logged-in host. SwiftPM's nested sandbox must be off inside this outer one.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
unset LOOPFLOW_NATIVE_TESTS
exec sandbox-exec -f "$ROOT/scripts/desktop-headless.sb" \
  swift test --disable-sandbox --package-path "$ROOT/swift" --no-parallel "$@"
