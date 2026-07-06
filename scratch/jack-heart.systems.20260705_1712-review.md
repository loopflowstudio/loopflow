# Gate Review

## What was implemented

Scheduled release note generation now falls back to deterministic mechanical notes when the agent-backed `release-notes` step cannot start because Claude, Codex, OpenCode, or another agent CLI is missing. The fallback uses the same release context: version, previous tag, release target, merged PRs, and archived `release/unreleased/DECISIONS.md`.

Docs in `docs/lfop.md` and `release/README.md` now state that scheduled release automation does not require a runner-local agent CLI.

## Key choices

The fallback is narrow. It only handles errors that mention an agent CLI and a missing binary/spawn condition. Other `release-notes` failures still fail the release, preserving the release contract instead of hiding broken narrative generation.

The existing mechanical note generator remains the single fallback implementation. It now accepts optional release decisions so the CI path keeps the same intent source as interactive notes.

## How it fits together

`run_release_notes_step` still invokes `lf --batch release-notes` first. On a missing-agent error, it calls `generate_release_notes`, writes `RELEASE_NOTES.md` through the existing helper, and lets the normal archive path copy it to `release/v<version>/NOTES.md`.

## Risks and bottlenecks

The missing-agent detector is string-based because the error comes from the child process boundary. It is intentionally conservative: a new missing-agent wording could still fail the release until the detector learns that wording.

Generated fallback notes are factual, not narrative. That is acceptable for scheduled patch releases; interactive/manual releases still get the agent-backed path when an agent CLI is available.

## What's not included

This does not install an agent CLI in CI, provision model credentials, or change the weekly release workflow. It keeps the existing workflow self-contained by making the release command resilient to the runner's missing agent CLI.

## Validation

- `cargo test -p loopflow release_notes --test release_tests`
- `cargo fmt --check`
- `cargo test -p loopflow --test release_tests`
- `cargo clippy -p loopflow -- -D warnings`
- `uv run python scripts/test.py` ran Rust and website; first website pass had one transient `Page.goto("/")` timeout, and the single failing test passed on immediate rerun.
- `cd website && uv run python dev.py test`: 61 passed, 3 skipped.
- `uv run python scripts/test.py --all`: Python, Rust, website, Swift, and e2e passed. Concerto UI failed locally before bootstrapping the UI test runner: `ConcertoUITests-Runner ... Early unexpected exit ... signal kill before establishing connection`. No assertion failure was reported.
