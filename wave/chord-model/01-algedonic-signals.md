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

## Status

Built so far:
- `WaveRun.repair_of` field in persistence stack (migration, struct, catalog, sqlite, postgres)
- `classify_repair_flow` — returns `debug` for all failures (ci-fix failures get `debug` to avoid loops)
- `create_repair_run` — creates a new run linked to the failed run, same worktree/branch
- `fail_run` — creates algedonic signal only when repair_of is set (repair already tried and failed)
- `execute_run_inner` — dispatches repair run when a first failure occurs (repair_of is None)
- Tests for classification and repair run creation

## Next: live demo

The repair dispatch path compiles and unit tests pass, but hasn't run end-to-end in lfd. The demo attempt exposed infra gaps:

1. **Dev lfd token isolation** — dev lfd and Concerto lfd fight over `~/.lf/session-token`. Need `LF_HOME` or similar to isolate dev instances.
2. **PR state sync** — after `ops: land --create-pr`, the run's snapshot doesn't reflect the PR. `check-ci` polling needs the run to have an open PR to find CI targets.
3. **Demo harness** — `scripts/dev-lfq` exists but needs stable token handling. Consider a `scripts/demo-algedonic.sh` that starts isolated lfd, creates wave, runs it, polls for CI failure, triggers check-ci, and shows repair/escalation.

Demo sequence when infra is ready:
1. `make-tests-fail` step breaks a test (haiku, proven to work)
2. `ops: land --create-pr` pushes PR (proven to work — PR #574 was created and CI failed)
3. `check-ci` detects failure → ci_failure trigger → ci-fix run
4. If ci-fix fails → `execute_run_inner` dispatches repair with `debug` flow
5. If repair also fails → `fail_run` creates algedonic attention item

## Done when

- Live demo: step failure → repair → escalation works end-to-end in lfd
- Step failure triggers headless repair attempt with `repair_of` set
- Failed repair creates algedonic attention item with error context
- CI webhook → ci-fix → escalation works end-to-end
- Successful repair resolves algedonic items
- Retry limit prevents infinite repair loops
- Concerto displays algedonic items and can launch interactive debug
