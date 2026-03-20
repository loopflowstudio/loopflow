# Review: worker-capacity waves, garden/VSM flows, and stacked UI/PM foundations

## What was implemented

This branch ships a stacked milestone across the core daemon, CLI, Python client, and Concerto UI:

1. **Wave execution now uses worker capacity instead of serialized mode.** `workers` is persisted in lfd storage, exposed through HTTP/Python/Swift models, and used by scheduling/activation logic.
2. **The chord-model vocabulary was reshaped around garden/wave/VSM planning flows.** Builtin steps and flows now use `garden/*`, `wave/*`, and `vsm/*` names, with updated docs and roadmap items describing the new planning rhythm.
3. **Flow expansion and routing were simplified.** `engine/flow.rs` and CLI flow rendering now collapse duplicated branch plumbing and support the new builtin flow layout.
4. **The stacked branch also carries prior shipped foundations** for terminal sessions / attention UI and PM bootstrap workflows, so the reviewer will see Swift/HTTP/PM surfaces alongside the chord-model changes.
5. **Gate polish fix:** branch renames now rename from the linked worktree first, which fixes the failing `wave_rename_renames_branch` path after `git worktree move`.

## Key choices

- **Canonicalize on `workers`, keep `serialized` as an input shim.** The persisted model and DTOs now describe capacity directly, while create/update handlers still map legacy `serialized` request fields to worker counts so existing callers do not break immediately.
- **Move governance into named garden/wave/VSM builtins instead of tend/chord drafts.** The branch deletes the temporary tend-specific chord authoring steps and replaces them with concrete scan/assess/mutate/review steps that match the updated chord-model docs.
- **Prefer flow-engine simplification over more one-off builtins.** The OR/XOR/parent-flow plumbing was collapsed in code so the new flow catalog can stay declarative instead of adding more bespoke parsing paths.
- **Handle linked-worktree branch renames directly in git helpers.** Renaming from the owning worktree is simpler and more reliable than probing for a failed repo-level rename and trying to recover afterward.

## How it fits together

The branch updates the builtin flow catalog and flow engine so loopflow can talk about planning in garden/wave/VSM terms, then threads the new execution model through lfd persistence, API DTOs, Python models, and Swift view models. The scheduler/executor consumes `workers` as a capacity cap, while the UI/API layers display the same state consistently. The gate fix closes the last failing rename edge case in the wave/worktree lifecycle.

## Risks and bottlenecks

- **Branch breadth:** the diff against `main` is very large and spans Rust, Python, Swift, docs, and roadmap files. Review is easiest commit-by-commit or by subsystem cluster.
- **Compatibility edge cases:** `serialized -> workers` mapping now lives in create/update handlers. Any caller bypassing those paths or assuming the old field is canonical could still drift.
- **Worktree lifecycle sensitivity:** wave renames and worker-capacity scheduling both depend on git/worktree state staying consistent across linked worktrees.
- **Concerto UI test instability:** the Swift package tests pass, but the full `xcodebuild test` command still crashes `ConcertoUITests-Runner` before bootstrap on this machine.

## What's not included

- The recursive chord-tree planning traversal described in `scratch/chord-model-worker-pools.md` is still design work; this branch ships the vocabulary and worker-capacity primitives, not a full tree-walk engine.
- No new primitive was added for “run these planning steps at each node in tree order.”
- The local `ConcertoUITests-Runner` bootstrap crash was reproduced twice during gate, but not fixed here.

## Validation

The design doc does not include a dedicated “done when” checklist, so this gate pass used the repo’s CI-style checks for the touched surfaces.

### Passed

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/ -q` → `115 passed`
- `tests/e2e/test_smoke.sh` → `PASS`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` → `16 passed`
- `swift test --package-path swift`

### Failed / needs follow-up

- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
  - failed twice locally with: `ConcertoUITests-Runner ... Early unexpected exit ... Test crashed with signal kill before establishing connection`.
