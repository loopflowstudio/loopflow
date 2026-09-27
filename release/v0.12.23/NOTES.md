# v0.12.23

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.23 updates the locked Python development dependency Ruff from 0.16.3 to 0.16.4. Contributors get fixes for lint diagnostics and Windows compatibility; the change since v0.12.22 is confined to `uv.lock`.

## More reliable Python checks

The Ruff update brings upstream fixes to the contributor toolchain, including a crash fix for older Windows CPUs and more accurate syntax diagnostics.

- Fixes Ruff's `InvalidInstruction` failure on Windows CPUs without `POPCNT` support.
- Reports semantic syntax errors in string type definitions as `F722`.
- Detects duplicate keyword arguments and parameters declared `nonlocal`.

## Small changes

- Ruff's editor server supports pull diagnostics for notebook cells and marks safe fixes as preferred.