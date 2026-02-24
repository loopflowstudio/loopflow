#!/usr/bin/env bash
set -euo pipefail

uv run python scripts/test_api_smoke.py
