# Review: reverse branch naming for worktrees

## What was implemented

Two related changes to worktree management:

1. **Reverse branch name parsing** — `parse_branch_name(branch, config)` decomposes a full schema branch (e.g. `jack-heart.mobile.20260225_1122`) back into its `{user}`, `{name}`, `{timestamp}`, and `{words}` components using a compiled regex derived from the schema pattern. `wave_name()` wraps this to extract just the `{name}`.

2. **Schema-aware worktree creation** — `create_with_schema()` now checks `origin/<input>` first. If the input matches an existing remote branch, it checks out that branch (with tracking) instead of minting a new one. Worktree directory names always use the wave component (`{name}`), not the full branch string.

Side changes: default branch schema drops `{words}` (now `{user}.{name}.{timestamp}`), and an "Ambition" section is added to LOOPFLOW.md.

## Key choices

- **Regex from schema, not hardcoded patterns.** `compile_schema` turns any schema string into a regex by mapping each placeholder to a character-class pattern. Custom schemas with `{words}` or `{date}` work without special-casing. Single-entry Mutex cache avoids recompilation in hot paths.

- **Greedy `{name}` with dot support.** The `{name}` placeholder matches `[a-z0-9._-]+`, which is greedy. With the default dot-delimited schema, the regex engine backtracks to let the unambiguous timestamp pattern (`\d{8}_\d{4}`) anchor the end. This allows names like `mobile.feature` to parse correctly.

- **`WorktreeBranch` enum over boolean flags.** The old `worktree_add` always created a new branch. The new signature takes `WorktreeBranch::New`, `::Track`, or `::Existing` — making the three git invocation modes explicit at the type level.

- **`rev_parse` to detect remote branches.** Cheaper than `git ls-remote` and works offline against the local ref cache. Requires a recent `git fetch`, which is the common case for worktree creation flows.

## How it fits together

```
User input (e.g. "jack-heart.mobile.20260225_1122")
  → create_with_schema()
    → rev_parse("origin/jack-heart.mobile.20260225_1122") → found
    → worktree_path_with_config() → wave_name() → "mobile" → ../loopflow.mobile
    → worktree_add(..., WorktreeBranch::Track { remote })
```

For short names like `"mobile"`, `rev_parse` fails, `format_branch_name` generates the full schema name, and `worktree_add` uses `WorktreeBranch::New`.

## Risks and bottlenecks

- **Regex cache is global single-entry.** Tests running in parallel with different schemas will thrash the cache. No correctness issue (compile_schema always produces the right regex), just redundant recompilation. Fine for now; only becomes worth expanding if many schemas are used concurrently.

- **`rev_parse` for remote detection depends on local refs being current.** If `origin/<branch>` exists remotely but the local repo hasn't fetched, `create_with_schema` won't see it and will try to create a new branch instead. This is the expected behavior (same as `git checkout`), but worth noting.

- **Greedy name matching could surprise with unusual schemas.** If a custom schema puts `{name}` before another dot-separated text placeholder (not timestamp), the greedy `{name}` would consume too much. In practice, the only ambiguous separator is `.` and the timestamp's `\d{8}_\d{4}` pattern resolves it. The design doc's follow-up to keep Concerto wave-name validation strict (`[a-z][a-z0-9-]*`) mitigates this.

## What's not included

- **Wave rename flow.** The design doc sketches a "Design a Wave" NUX where `design` renames the wave once intent is clear. That's deferred — this PR only adds the parsing and worktree-creation primitives it depends on.

- **Concerto UI changes.** No Swift changes in this branch. The "Design a Wave" button and random name generation are future work.

- **Branch rename on wave rename.** `wave_name()` parses branch names but doesn't yet drive automatic branch renaming when a wave is renamed. Existing `branch_rename` + `worktree_move` tests show the mechanics work; wiring them through a rename flow is a follow-up.
