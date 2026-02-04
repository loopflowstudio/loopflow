# Open Questions

- Smoke test uses `lf-prompt` instead of `lf run ... --dry-run` because neither Rust nor Python CLI exposes a dry-run flag. Is this acceptable, or should we add a CLI dry-run mode?
- Release workflow bundles only the `lf` binary into the wheel. Should `lfd` be bundled as well in this phase?
- Wheel build now packages all platform `lf` binaries into a single `py3-none-any` wheel (selected at runtime). Do we want platform-specific wheels instead?
