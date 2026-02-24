#!/usr/bin/env bash
set -euo pipefail

uv run pytest tests/e2e/test_api_smoke.py -v
