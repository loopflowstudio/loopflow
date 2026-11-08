# Documentation audit

Jack Heart: "can you take a thorough pass on ALL the docs"

Jack Heart: "also make sure we are lf sync'ed"

2026-11-08: repository-wide scan covered 144 maintained Markdown files, including
user/developer guides, module READMEs, builtins, integration guidance, Swift design,
release instructions and CI. Published release history, licenses, fixtures and
Wave memory remain historical/source material. Read current source to reconcile
server revision/limits, creation payloads and results, provider/terminal recovery,
Desktop transports, Task graph position/return behavior, and planning ownership.

Checks recorded during this pass:
- `uv run python scripts/check_architecture.py --json`: passed after adding the
  creation receipt owner (79 tables after sync).
- Website portable architecture and README/index consistency tests: 2 passed.
- All 33 documentation routes render with an H1 via Starlette TestClient.
- Relative-link/anchor scan found and repaired release and CI links, data's
  installation anchor, and the repo command anchor.
- `git diff --check`: passed.
- `LF_BIN=$PWD/target/debug/lf uv run python website/dev.py sync-docs`: 33 routes
  regenerated from the merged candidate; every linked command-reference anchor
  resolves. The installed CLI still generates the old ops-nested tree.
- `cargo test -p loopflow --test integration documented_commands::`: 4 passed
  on the merged tree. Candidate path is recorded in `loo-810-sync.md`.

Recovery now offers Task title, state, request, Graph history and conversations,
or Session history, through an Inspect action on the original Machine. Unknown
launches remain unresolved and cannot be dismissed as a confirmed result.

Gate retains the merged-tree served-operation/schema checks and native demo
review. The implementation plan records focused checks and remaining acceptance.

The docs audit edits are in the checkout; the first batch is checkpointed by the
implementation agent before sync. Later prose edits remain alongside active code.
