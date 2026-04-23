## Try it!

```bash
cat release/README.md
cargo run --quiet --bin lf -- op release run --help
cargo test -p loopflow ops::release::tests::promote_unreleased_renames_dir
cargo test -p loopflow ops::release::tests::archive_release_notes_copies_root_to_version_dir
cargo test -p loopflow --test golden_prompt
```

What you'll see:
- `release/README.md` explains the new `release/unreleased/DECISIONS.md` → `release/v<version>/` flow
- `lf op release run --help` still exposes the same entrypoint, now backed by artifact promotion and note archival
- the release tests prove promotion, fallback, collision handling, and note archival
- the golden prompt test proves the built-in prompt bundle and checked-in goldens stay in sync

## Intent

Keep release intent close to the code instead of losing it in chat transcripts and PR titles. This branch adds a first-class release decisions ledger, archives versioned notes beside versioned decisions, and updates the release-notes prompt so release summaries lead with intent when the ledger exists and gracefully fall back to merged PR history when it does not.

## Assumptions

- Release cycles may or may not maintain `release/unreleased/DECISIONS.md`; the workflow must succeed either way.
- The repo-root `RELEASE_NOTES.md` remains the always-latest artifact, while `release/vX.Y.Z/NOTES.md` is the per-version snapshot.
- Prompt goldens are the source of truth for built-in prompt rendering, so prompt text changes must update those fixtures.

## Key decisions

- Promote `release/unreleased/` during `lf op release run`, not at some later manual cleanup step.
- Treat `DECISIONS.md` as primary release-note input when present; merged PR data fills gaps instead of driving the whole narrative.
- Correct repo docs to match the implementation timing and fallback behavior, and fix `TESTING.md` to point at the real golden refresh command.

## Not included

- Automatic recreation of `release/unreleased/DECISIONS.md` after a release
- Additional automation for writing decision entries outside interactive agent guidance
