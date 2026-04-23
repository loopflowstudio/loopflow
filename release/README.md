# Release artifacts

```bash
cat release/unreleased/DECISIONS.md
lf op release run patch
find release -maxdepth 2 -type f | sort
```

Use `release/` to keep the rationale and notes for each shipped version close to the code.

| Path | What it does |
|------|--------------|
| `release/unreleased/DECISIONS.md` | Append-only ledger for release-worthy intent and policy decisions during the current cycle |
| `release/vX.Y.Z/DECISIONS.md` | Archived copy of that ledger for one shipped version |
| `release/vX.Y.Z/NOTES.md` | Archived copy of the release notes generated for that version |
| `RELEASE_NOTES.md` | Always-latest release notes at the repo root |

Append to `release/unreleased/DECISIONS.md` only when the change captures durable intent: policy choices, scope calls, paths not taken, or decisions a contributor would cite months later. Skip bug-fix churn and mechanical edits.

Interactive runs may append those decisions as they happen. Headless runs do not. At tag time, `lf op release run` renames `release/unreleased/` to `release/v<version>/`, uses `DECISIONS.md` to shape the narrative notes, and writes the final notes to both `RELEASE_NOTES.md` and `release/v<version>/NOTES.md`.
