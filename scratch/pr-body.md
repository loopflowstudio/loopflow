## Evaluate

```bash
lf task watch ISSUE --json
lf task output ISSUE --json
# Save next_cursor, then continue without putting its contents in argv:
lf task output ISSUE --json --cursor /path/to/cursor
uv run python scripts/test.py --reuse-passing
```

Watch returns persisted invocation plans, distinct attempts, exact transitions,
and Task-attributed Runs without needing a worker or worktree. Output returns
ordered source groups with native tool correlation, revisions, and explicit gaps.
Reading leaves the provider client unchanged.

Gate status: **not ready to ship**. The affected runner stopped before tests at
resource preflight: the active main build was 19.1 GiB against its 12 GiB limit;
documented recovery could not remove an active build. Architecture, Swift
boundary, and formatting checks passed. No fresh Clippy or affected-suite pass
is claimed. Concurrent snapshot edits require validation after the tree settles.

## Why it matters

Completed and restarted Tasks need inspectable execution history. Retaining the
actual selected plan and exact Run bindings avoids reconstructing history from
changed YAML or repeated skill names. Passive native readers preserve the
original Session client while making its recorded output available to shared
Task inspection.

## What changed

- Record flow facts atomically with position changes, without parent-controller
  notifications or new execution authority.
- Fold plans, attempts, Iterate/retry edges, settlement, and auxiliary Runs in
  shared Rust reads; expose typed CLI and Swift contracts with DTO fixtures.
- Read journal, Claude/Codex JSONL, and mutable OpenCode sources using exact
  recorded provenance, stable identities, revisions, and explicit gaps.
- Carry continuation through files/stdin and query-scoped Swift temporary files.
  Keep source labels, records, and availability together.

## Risks / Not included

The Mac Watch tab, navigation/filtering/Follow live behavior, independent
history/live cursors, bounded discovery, complete autonomous capture, and the
configured end-to-end demonstration are **still required work in this PR**.
Summary-only journal gaps do not satisfy complete capture. Claude/OpenCode live
proof and arbitrary OpenCode history-rewrite coverage remain incomplete.

Manifest failures preserve healthy output, but directory enumeration failures
still abort discovery and can affect older shared Run readers. Repair and prove
partial-directory recovery before shipping. Snapshot and discovery currently
scan all retained evidence; do not wire them to one-second polling yet.

No multi-Task dashboard, graph editing, or expanded remote transport is planned.
