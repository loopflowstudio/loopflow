## Try it!

Review this branch in subsystem clusters first:

```bash
git log --oneline main..HEAD | sed -n '1,20p'
```

Run the core validations that are green on this branch:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run pytest python/tests/ -q
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
swift test --package-path swift
cargo test -p loopflow --test wave_worktree_tests wave_rename_renames_branch -- --exact
```

What you should see:
- Rust/Python/e2e validation stays green.
- The targeted worktree regression test now passes because branch renames happen from the linked worktree.
- Swift package tests pass.

If you want to reproduce the only red local check, run:

```bash
cd swift
xcodegen generate
xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

Locally, that still fails with `ConcertoUITests-Runner ... Early unexpected exit ... signal kill before establishing connection`.

## Intent

Ship the chord-model transition from serialized waves and tend/chord drafting toward worker-capacity execution and named garden/wave/VSM planning flows, while keeping the already-stacked terminal-session, attention, PM bootstrap, and Concerto surface work coherent in one reviewable milestone. The branch updates the daemon, CLI, clients, and docs so `workers` is the execution primitive and the builtin flow catalog matches the new planning language.

## Assumptions

- Existing callers may still send `serialized`, so the HTTP handlers continue to translate that input into a worker count while the stored model and DTOs treat `workers` as canonical.
- Reviewers are best served by reading this as a stacked milestone: terminal sessions / attention UI, PM bootstrap, governance-flow renaming, then worker-capacity scheduling.
- The local macOS UI-test bootstrap crash is environmental or pre-existing enough that it was documented instead of “fixed” blindly during gate.

## Key decisions

- Persist and display `workers` end-to-end instead of recomputing serialization state indirectly.
- Delete the temporary tend chord-authoring steps and replace them with builtins that match the shipped garden/wave/VSM model.
- Simplify OR/XOR/parent-flow plumbing in the flow engine so the expanded flow catalog stays declarative.
- Rename linked-worktree branches from the owning worktree first, which removes a flaky `git branch -m` edge case after `git worktree move`.

## Not included

- No recursive tree-walk execution primitive for full chord planning yet.
- No fix for the local `ConcertoUITests-Runner` bootstrap crash; it remains the one red validation command in this gate pass.
