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
- `website/dev.py sync-docs` with the installed CLI generates an OLD ops-nested
  command tree, so canonical PR/Machine/account links cannot be checked from it.
  The branch has flattened command families. Regenerate using the built branch
  CLI and check command anchors before gate. No binary exists at target/debug/lf
  yet; check/clippy alone do not produce it.

Remaining verification for the implementation/gate build:
- Run `documented_commands` and the changed served-operation/schema fixture checks
  against the merged tree, using the same Cargo sequence as implementation.
- Use the branch CLI for website command-reference generation, then check links.
- Reconcile creation-recovery presentation with the user docs: the UI must expose
  the recovered Task/Session for inspection, not merely a request ID. Preserve
  unknown launches, original destinations and safe same-payload continuation.

The docs audit edits are in the checkout; the first batch is checkpointed by the
implementation agent before sync. Later prose edits remain alongside active code.
