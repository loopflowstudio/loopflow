# Sandbox Smoke Test and CLI Compatibility

Picked from `wave/sandboxes/01-integration-and-validation.md`.

## Status

Executor compatibility is covered in tests (`create`, `exec`, `stop`, `rm`, `ls`, `inspect` command shape). The platform script now fails fast when host sandbox plugins do not expose the required lifecycle commands.

## Scope

### Smoke tests (done)

`sandbox_` test module in `rust/loopflow/src/lfd/executor/sandbox.rs`:
- [x] Startup probe passes (or fails gracefully with clear diagnostics)
- [x] Claude agent runs end-to-end through sandbox executor
- [x] Cleanup removes sandbox on normal completion
- [x] Cleanup removes sandbox on timeout/terminate
- [x] Orphan recovery identifies and removes `lf-*` sandboxes

### Platform validation script

`scripts/test_sandbox_platforms.sh` validates full lifecycle on real environments:
- Startup probe (create + exec + rm)
- Context file sync (.lf/logs/ visible inside sandbox)
- Gemini CLI availability in claude template
- Cleanup verification (no orphaned lf-* sandboxes)

### CI integration

`sandbox-smoke` job in `.github/workflows/ci.yml` with probe-gate pattern. Skips gracefully when sandbox plugin unavailable.

## Done when

- [x] CLI compatibility confirmed
- [x] `sandbox_` smoke tests pass in CI alongside existing `docker_` tests
- [ ] Platform validation script passes on macOS
- [ ] Context file sync verified via platform script
- [x] Gemini path validated or documented
- [x] CI job added
