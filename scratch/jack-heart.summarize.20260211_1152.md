# Wave Area Summaries

One combined summary per wave, generated at wave start, kept fresh as the wave runs.

## What to build

Waves get an auto-generated summary of their area that loads into every step's context. Generated when the wave starts, proactively refreshed via git hooks, with a staleness check as safety net.

## Data structures

```sql
-- New DB migration
CREATE TABLE summaries (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id),
    content TEXT NOT NULL,
    source_hash TEXT NOT NULL,
    token_budget INTEGER NOT NULL,
    model TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

```rust
struct Summary {
    id: LfdId,
    wave_id: LfdId,
    content: String,
    source_hash: String,
    token_budget: usize,
    model: String,
    created_at: OffsetDateTime,
}

// Store trait additions
fn get_summary(&self, wave_id: &LfdId) -> Result<Option<Summary>>;
fn upsert_summary(&self, summary: &Summary) -> Result<()>;
```

## Generation

Run summarize as an **internal step** — same agent machinery as normal steps, but not part of the user's flow.

1. Compute source hash via `git ls-tree` on the wave's areas
2. If stored hash matches: skip (summary is fresh)
3. Gather source content from wave's areas
4. Build prompt from builtin `ops/summarize` template with `{token_budget}` and `{content}`
5. Run agent in the wave's worktree
6. Agent writes summary to `.lf/summary.md`
7. Read that file back, store content + source hash in DB

The summarize step prompt needs updating to include a "write your output to `.lf/summary.md`" instruction.

```rust
// executor.rs
async fn ensure_summary_fresh(&self, wave: &Wave, run: &WaveRun) -> Result<()> {
    let current_hash = hash_areas(&run.worktree, &wave.area)?;
    if let Some(existing) = self.store.get_summary(&wave.id)? {
        if existing.source_hash == current_hash {
            return Ok(()); // fresh
        }
    }
    // Stale or missing — run summarize
    self.run_internal_summarize(wave, run).await?;
    Ok(())
}
```

## Source hashing

```rust
fn hash_areas(worktree: &str, areas: &[String]) -> Result<String> {
    // git ls-tree -r HEAD -- <areas> | sha256
    // Fast, no file reads, tracks content changes
}
```

`git ls-tree` outputs blob hashes for every file in the area. Hash that output to get a single fingerprint. Runs in ms even on large repos.

## Keeping summaries fresh

**Proactive: git post-commit hook.** When lfd sets up a wave's worktree, install a post-commit hook that notifies lfd. lfd checks if area files changed and queues re-summarization if so.

```bash
# .git/hooks/post-commit (installed by lfd)
curl -s http://localhost:$LFD_PORT/api/hooks/post-commit \
  --data '{"wave_id": "$WAVE_ID", "worktree": "$WORKTREE"}'
```

lfd endpoint:
1. Compute new source hash for the wave's areas
2. Compare to stored hash
3. If stale: queue internal summarize step (non-blocking, runs in background)

**Safety net: staleness check.** Before each step in `execute()`, call `ensure_summary_fresh()`. If the hook missed something (or isn't installed), this catches it.

## Context assembly

```rust
// In gather_context(), replace the summaries TODO.
// Requires passing store access into gather_context.
if let Some(wave_id) = &opts.wave_id {
    if let Some(summary) = store.get_summary(wave_id)? {
        components.summaries.push(Document {
            path: format!("wave-summary"),
            content: summary.content,
            category: "summary".to_string(),
        });
    }
}
```

`gather_context` currently doesn't have store access. Either:
- Pass `store: Option<&dyn RunStore>` into `GatherContextOpts`
- Or load summary in `build_step_prompt` and inject it into components after `gather_context` returns

The second is simpler — keeps `gather_context` pure (filesystem-only) and handles summary injection at the executor level.

## Constraints

- `gather_context` is used by both `lf` (CLI, no DB) and `lfd` (daemon, has DB). Summary loading must be optional — only when a store is available.
- Source hash must be fast. `git ls-tree` is the right tool.
- The builtin summarize prompt exists. Extend it with file-write instruction, don't replace it.
- Summary trimming priority already exists in `trim_context_with_breakdown` — summaries are dropped before docs, after area_docs.

## Implementation sequence

1. **DB migration** — add `summaries` table, `Summary` type, store methods
2. **Source hashing** — `hash_areas()` using `git ls-tree`
3. **Internal step runner** — `run_internal_summarize()` that runs the agent and reads back `.lf/summary.md`
4. **Hook into executor** — call `ensure_summary_fresh()` before each step in `execute()`
5. **Context loading** — inject summary into `PromptComponents` in `build_step_prompt`
6. **Git hook** — install post-commit hook in worktree, add lfd endpoint
7. **Update summarize prompt** — add file-write instruction

## Done when

1. Wave starts → summary generated and stored in DB before first step
2. Wave steps include summary in context (visible in prompt log)
3. After a step modifies area files, next step gets refreshed summary
4. `git ls-tree`-based hash detects staleness correctly
5. Post-commit hook triggers background re-summarization
