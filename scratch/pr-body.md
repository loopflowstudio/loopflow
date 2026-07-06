## Try it!

```bash
cargo test -p loopflow release_notes --test release_tests
```

The new test stubs `lf --batch release-notes` to fail with a missing Claude CLI error. `lf op release notes` falls back to deterministic notes, preserves `release/unreleased/DECISIONS.md` content, writes `RELEASE_NOTES.md`, archives `release/v<version>/NOTES.md`, and removes `release/unreleased/`.

## Intent

Weekly scheduled patch releases should not depend on a human-installed Claude, Codex, or OpenCode CLI on the GitHub runner. This keeps the agent-backed release-note path when available, but makes the scheduled path self-contained when the runner has no agent CLI.

## Assumptions

The release command can still reach `gh` and collect merged PRs. Missing-agent fallback is only for the `release-notes` child step failing before it can run because an agent binary is absent.

## Key decisions

The fallback is deliberately narrow: missing agent binary/spawn errors produce mechanical notes, while other release-note failures still fail the release. Mechanical notes use the same release context as the agent path, including archived decisions, so the archive contract stays intact.

## Not included

This does not install an agent CLI in CI or add model credentials to the weekly workflow.

## Validation

- `cargo fmt --check`
- `cargo clippy -p loopflow -- -D warnings`
- `cargo test -p loopflow --test release_tests`
- `uv run python scripts/test.py --all`

`scripts/test.py --all` passed Python, Rust, website, Swift, and e2e. The local Concerto UI runner failed before bootstrapping with `Early unexpected exit ... signal kill before establishing connection`; no test assertion failed.
