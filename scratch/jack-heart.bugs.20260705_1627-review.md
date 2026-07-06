# Gate Review: wave reset and ghost reclaim

## What was implemented

Added `lf op reset-waves` as an operator fresh-start command. It lists every
`lf-*` tmux session, confirms in an interactive terminal unless `-y/--yes` is
passed, kills those sessions, and clears stale `wave/*/.wave-endpoint` pointers
from the main repo.

Concerto's macOS wave launcher now treats an existing tmux session with no live
wave endpoint as a reclaimable ghost. It grace-probes for a live endpoint before
killing the stale session name, then launches the wave normally.

## Key choices

- A probed live endpoint is the only hard block for Concerto launch. Raw pointer
  files and tmux session names both survive crashes, so blocking on either would
  keep Start dead after a crash.
- The macOS launcher waits up to three one-second probes before reclaiming an
  existing session. That covers the mid-boot window where `lf wave` has not yet
  published `.wave-endpoint`.
- `lf op reset-waves` is a direct CLI command, not an executable flow op item.
  It has broad side effects and is explicitly rejected by `execute_parsed_ops`.
- Gate cleanup removed generated `.lf/metrics/ops.jsonl` and direct
  `wave/*/MEMORY.md` edits from the effective diff; memory is server-owned.

## How it fits together

The Rust CLI owns the bulk operator reset path. Concerto owns the single-wave UI
launch path. Both rely on the same invariant: a live server endpoint means the
wave is owned; a tmux session without a live endpoint is stale process state.

## Risks and bottlenecks

- `lf op reset-waves` intentionally kills every `lf-*` tmux session on the
  machine. The confirmation prompt protects interactive use, while `--yes` is
  available for automation.
- Concerto's grace probe adds up to two seconds only when a tmux session already
  exists. Normal launches still do one endpoint probe.
- Local Concerto UI validation did not fully complete: the Xcode UI test runner
  was killed before bootstrapping. The app/unit tests in that job passed before
  the UI runner bootstrap failure.

## What's not included

- No registry mutation is added to `lf op reset-waves`; lfd reconciles stale
  rows on boot.
- No user-facing README changes. The command appears in generated CLI help, and
  the branch is primarily an operator/launcher fix.

## Validation

- `cargo fmt`
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `uv run python scripts/test.py`: passed Rust and Swift changed-aware suites.
- `uv run python scripts/test.py --all`: Python, Rust, website, Swift, and e2e
  passed; Concerto UI failed during runner bootstrap with:
  `ConcertoUITests-Runner encountered an error (Early unexpected exit, operation
  never finished bootstrapping ... Test crashed with signal kill before
  establishing connection.)`
