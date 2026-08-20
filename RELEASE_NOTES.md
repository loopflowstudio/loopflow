# v0.12.10

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.10 tightens the execution boundaries that keep autonomous work trustworthy. Task runs now prove they can deliver before reserving execution, resident Waves recover safely when their flow definitions change, and browser jobs no longer borrow ownership of a person's Chrome session. The Mac app also runs control activity through the `lf` built and shipped with it, removing another source of runtime drift.

## Stop impossible Task deliveries before execution

Delivery Tasks now carry an explicit outcome contract, and Loopflow validates that the selected lifecycle can converge before it starts. A delivery flow must be able to make autonomous progress, include an implementation-capable skill, and end with `lf pr land -c`; intentionally non-implementing work must opt into `--design-only` (#1207).

- `lf task run DES-123` validates the delivery lifecycle, managed provider credentials, writable linked-Git state, and writable control-store state before reserving a Run.
- `lf task run DES-124 --design-only --loop gate --finally ship` records that implementation is intentionally out of scope instead of weakening delivery checks for every Task.
- Permission, control-authority, credential, and network failures become durable, user-visible blockers rather than retries or apparent success.
- Worktree writer authority is bound to the exact live Run, Turn, or Ask, while stale writers can be reclaimed safely.
- Task completion requires reviewer-facing PR copy for the current head SHA and an armed auto-merge settlement, so an older review state cannot satisfy a newer delivery.
- `lf task status` and the roadmap DTOs expose whether a Task is a delivery or design-only outcome.

## Continue Waves safely after flow edits

A resident Wave now checks its journaled playhead against the current flow definition before opening the next body. If steps were renamed, reordered, reshaped, or given different policies while the resident was idle, Loopflow records one durable reset and resumes from the current root instead of replaying an obsolete cursor (#1202).

- Root, nested, and queued plans are compared by step name, kind, order, policy, and shape.
- A reset clears stale cursors, iterations, nested invocations, and queued continuations while preserving pending chat and inbox input.
- A body already in progress remains pinned to the plan that started it; reconciliation waits until that body finishes.
- Reset events are journaled and narrated, making recovery visible and replayable across restarts.

## Capture pages without taking over Chrome

`lf screenshot` captures URLs or local HTML through an isolated `chrome-headless-shell` profile. Screenshot work now has a bounded process and output boundary, while provider authentication uses the same explicit ownership model without disturbing the user's foreground browser (#1204).

- Capture a page with `lf screenshot page.html -o page.png`, or set a viewport with `--width` and `--height`.
- Loopflow validates the PNG before publishing it atomically, preserving an existing output file when capture fails or is interrupted.
- Screenshot jobs have a 30-second lifetime and reap their complete browser process group when the caller exits.
- Provider authorization and token refresh processes receive the same descendant-cleanup guarantees.
- Claude authorization detection is passive: it no longer changes focus or sends synthetic keyboard input.
- Browser approval waits report progress and end at provider expiry or after ten minutes.

## Keep Mac control activity in lockstep

Generated Mac app builds now bundle and sign validation-only `lf` and `lfd` helpers from the same checkout, then use that exact `lf` for local process activity and agent controls. This removes PATH lookup and capability probing, preventing stale development binaries from causing wire-format mismatches (#1205).

- Local activity queries and agent actions resolve only through the bundled helper.
- A missing or non-executable bundled `lf` now produces a direct failure instead of falling back to another installation.
- macOS CI builds the dedicated Rust helper target and verifies that bundled `lf ps --json` activity decodes in the app.

## Operational notes

- Install the screenshot browser once with `playwright install --only-shell chromium` before using `lf screenshot`.
- Custom Task lifecycle definitions that deliver code must include autonomous progress, an implementation-capable skill, and terminal `lf pr land -c` settlement. Mark intentionally non-implementing Tasks with `--design-only`.
- Mac development builds now require the Rust toolchain to produce the bundled control helpers; `uv run python scripts/loopflow-dev.py run-debug` builds the configured debug path.

## Small changes

- Updated `fancy-regex` from 0.17.0 to 0.18.0.
- Refreshed 16 Rust dependencies, including `clap`, `serde`, `rusqlite`, and Tokio utilities.
- Updated the Python development lock to Ruff 0.16.0.
- Updated the architecture-drift workflow to `actions/upload-artifact` v7.
