# 01: Algedonic Signals

**Finish line:** When any step fails, lfd attempts headless repair. When repair fails, an algedonic signal surfaces in the attention queue. The CI failure path works end-to-end as the proving slice.

## What to build

1. **Post-run failure hook** — generic, in the executor. After any run completes with `Failed`:
   - If `repair_of` is set: this was a repair attempt that failed → create algedonic signal
   - If `repair_of` is None: first failure → classify error → launch repair run in same branch/worktree

2. **`WaveRun.repair_of: Option<LfdId>`** — explicit link to the run being repaired. Distinguishes repair attempts from normal runs without parsing reason strings.

3. **Error classifier** — examines failed run, returns repair strategy. CI failures → `ci-fix`. Everything else → `debug` with error context. Extensible as we learn more error classes.

4. **Algedonic signal creation** — `AttentionItem(kind: Algedonic)` with `chord_id` (target chord for routing). Context JSON carries original error, repair attempt logs, branch state. Emits `Event::AttentionCreated`.

5. **Auto-resolve on success** — when the underlying problem is fixed (CI passes, step succeeds), resolve pending algedonic items for that wave + branch.

6. **Interactive fallback** — clicking an algedonic item in Concerto launches interactive `debug` in the failing worktree with error context pre-loaded.

7. **Retry limit** — 3 repair attempts per (wave, branch) incident. After that, algedonic signal without further retry.

## CI failure proving slice

The CI path touches every layer and most pieces already exist:
- Webhook → `Event::CiFailure` → `ci_failure_handler` → activation → `ci-fix` run
- Missing: post-run hook detecting ci-fix failure, algedonic signal creation, auto-resolve on CI success

## Done when

- Step failure triggers headless repair attempt with `repair_of` set
- Failed repair creates algedonic attention item with error context
- CI webhook → ci-fix → escalation works end-to-end
- Successful repair resolves algedonic items
- Retry limit prevents infinite repair loops
- Concerto displays algedonic items and can launch interactive debug
