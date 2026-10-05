#!/usr/bin/env bash
# Build Desktop and run tests with WindowServer connections denied, even on a
# logged-in host. SwiftPM's nested sandbox must be off inside this outer one.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
unset LOOPFLOW_NATIVE_TESTS
export GIT_ALLOW_PROTOCOL=file
build_args=()
remaining=("$@")
while [ "$#" -gt 0 ]; do
  case "$1" in
    --jobs|-j|-Xswiftc|-Xlinker|-Xcc|--configuration|-c|--scratch-path)
      build_args+=("$1" "$2"); shift 2 ;;
    *) shift ;;
  esac
done
swift build --build-tests --package-path "$ROOT/swift" ${build_args[@]+"${build_args[@]}"}
exec uv run --no-sync python "$ROOT/scripts/test_network.py" --desktop \
  swift test --skip-build --disable-sandbox --package-path "$ROOT/swift" --no-parallel ${remaining[@]+"${remaining[@]}"}
