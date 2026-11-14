#!/bin/bash
# Stable entry point used by the installed launchd job. No package resolution or
# uv cache lock is needed: the mechanical runner uses only Python's stdlib.
set -eu
exec /usr/bin/python3 "$(dirname "$0")/disk_cleanup.py" "$@"
