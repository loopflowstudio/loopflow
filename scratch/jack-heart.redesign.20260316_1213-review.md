# Branch Review — jack-heart.redesign.20260316_1213

## What was implemented

This branch turns chords into ordinary waves.

- Added the redesign wave set on disk, including a new `wave/redesign/` chord-wave whose `area` points at the four member waves.
- Removed chord-specific server, store, and Python client/API infrastructure so waves are the only first-class runtime object.
- Added `scripts/bootstrap-redesign.py` to register the four redesign waves plus the redesign chord-wave with `lfd`.
- Updated docs and READMEs to describe the new chords-as-waves model.
- Added tests covering bootstrap behavior, CLI rendering, migration teardown, and wave-config-derived mode loading.
- Polished the bootstrap path so it registers the canonical repo root instead of `.` and exports `revoke_connection_tokens` from the Python API again.
- Marked the redesign wave configs as `mode: manual` so bootstrap creates dormant waves instead of immediately starting tend/build loops.

## Key choices

- **Chords are waves, not a parallel data model.** The branch deletes chord tables, routes, DTOs, and Python wrappers instead of deprecating them in place.
- **Membership lives in `area`.** The redesign chord-wave points at `wave/<name>/` directories instead of storing separate membership rows.
- **Bootstrap resolves the git common dir.** Running the script from a worktree now registers the canonical repo root, not the caller's `.` path.
- **Redesign waves bootstrap in manual mode.** This keeps registration idempotent and avoids auto-start behavior while the redesign machinery is still being assembled.

## How it fits together

`wave/<name>/<name>.yaml` now defines both ordinary waves and chord-waves. `create_wave` reads those configs, persists a normal wave record, and the redesign bootstrap script registers five waves through the existing wave API. The server and Python layers no longer know about a separate `Chord` type; Concerto and future tend logic infer hierarchy from wave `area` paths.

## Risks and bottlenecks

- Existing general-purpose API docs still show `repo="."` examples; this branch only hardens the redesign bootstrap path.
- Some older roadmap docs outside the redesigned wave set still mention legacy chord UI concepts as future work; they are design artifacts, not active runtime behavior.
- The branch intentionally removes chord CRUD outright, so any out-of-tree consumer of `/v0/chords` would break.

## What's not included

- Tend flow implementation (`scan-waves`, `assess`, `propose`, `apply`).
- Chord-wave detection/default-flow runtime behavior beyond the redesign configs.
- Concerto graph rendering or inline member presentation in `lfq show`.
- Letta integration and wave mutation APIs.

## Validation

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/ -q`
- Temp-daemon bootstrap smoke check in an isolated checkout:
  - `uv run python scripts/bootstrap-redesign.py`
  - `uv run lfq show redesign`
  - Result: all five waves created; `redesign` reported `flow: tend`, `status: idle`, the four `wave/.../` area entries, and an absolute repo path instead of `.`.
