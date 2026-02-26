# wave_name: reverse branch naming for worktrees

## What it does

Normalizes worktree naming around the wave identity (`{name}`) instead of full branch metadata. When given a full schema branch as input, `create_with_schema` checks `origin/<input>` first — if present, it checks out that branch with tracking instead of minting a new one.

```
lf ops wt create jack-heart.mobile.20260225_1122
  → rev_parse("origin/jack-heart.mobile.20260225_1122") → found
  → wave_name() → "mobile"
  → worktree at ../loopflow.mobile, tracking origin

lf ops wt create mobile
  → rev_parse fails → format_branch_name generates schema branch
  → worktree at ../loopflow.mobile, new branch
```

## Key decisions

- **Regex from schema, not hardcoded patterns.** `compile_schema` turns any schema string into a regex by mapping each placeholder to a character-class pattern. Custom schemas with `{words}` or `{date}` work without special-casing. Single-entry Mutex cache avoids recompilation.

- **Greedy `{name}` with dot support.** `{name}` matches `[a-z0-9._-]+`. The unambiguous timestamp pattern (`\d{8}_\d{4}`) anchors the end via backtracking. Names like `mobile.feature` parse correctly.

- **`WorktreeBranch` enum over boolean flags.** The old `worktree_add` always created a new branch. New signature takes `WorktreeBranch::New`, `::Track`, or `::Existing` — three git invocation modes explicit at the type level.

- **`rev_parse` for remote detection.** Cheaper than `git ls-remote`, works offline against local ref cache. Depends on a recent `git fetch`, which is the common case.

## Known risks

- **Regex cache is global single-entry.** Parallel tests with different schemas thrash the cache. No correctness issue, just redundant recompilation.

- **Greedy name matching with unusual schemas.** If a custom schema puts `{name}` before another dot-separated text placeholder (not timestamp), the greedy `{name}` consumes too much. Mitigated by keeping Concerto wave-name validation strict (`[a-z][a-z0-9-]*`).

## Validation

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test -p loopflow parse_branch_name
cargo test -p loopflow wave_name
cargo test -p loopflow create_with_schema_uses_existing_remote_branch_and_wave_worktree_name
```

## Follow-ups

- **"Design a Wave" NUX** — Replace wave creation in Concerto with a single button that generates a random word-pair name (e.g. `aurora-fugue`), launches the `design` step, then renames the wave once intent is clear. Random name gets you into flow immediately; design earns the real name.

- **Wave rename flow** — Wire `wave_name()` parsing through automatic branch rename + worktree move when a wave is renamed. The primitives (`branch_rename`, `worktree_move`) already work; the wiring is what's missing.

- **Concerto validation** — Keep wave-name input validation strict (`[a-z][a-z0-9-]*`) so branch parsing stays unambiguous.
