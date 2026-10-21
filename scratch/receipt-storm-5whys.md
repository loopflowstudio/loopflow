# Receipt storm: why Jack's laptop slowed and `lf` hit "database is locked"

## What failed

On 2026-10-21 around 12:40 PDT Jack's laptop was sluggish and `lf resume -a claude`
failed with `sqlite error: database is locked` after a 15 s wait. `lf pr land
--wait-and-fix` failed the same way. Load average was 65–124 on 16 cores.

Desktop's `lf monitor work --watch --json` (pid 17547, started by Loopflow.app)
ran at ~98% CPU continuously. A `sample` showed it inside
`journal::read_process_receipt_files_at` → `ProcessReceipt` deserialize.
`~/.lf/runtime/exec-processes` held 237,236 receipt files (927 MB).

**Recovery (done, reversible):** 231,887 receipts with a dead PID and mtime older
than a day were moved to `~/.lf/runtime/exec-processes-archive-20261021`.
5,357 remain. The monitor then fell to 3.7% CPU. Load stayed high because of
concurrent agent work (Swift builds for LOO-531, Rust tests for LOO-551 and
tmux-test, seven running Task flows). That load is separate and needs no code fix.

## Causal chain

1. **The monitor burned a core.** The Activity part rebuilds every 2 s
   (`ACTIVITY_CLOCK`, `rust/loopflow/src/lf/commands/work_watch.rs:46`) and on
   store bursts. Each rebuild calls `top::load_snapshot`, which reads and parses
   every receipt (`journal/mod.rs:1206`). Parsing 237k files takes longer than
   2 s, so the loop never rested. Its constant reads competed with writers on
   the 9 GB store, which is consistent with the lock timeouts. That link is
   inferred; lock holders were not traced.

2. **237k receipts existed because almost none were ever removed.** A receipt is
   written at admission for every `lf` process (`journal/mod.rs:869`). It is
   deleted only after a *completion* row inserts successfully
   (`journal/mod.rs:628`). `prune_dead_process_receipts`
   (`lf/commands/top.rs:219`, run by `lf disk clean`) also requires a recorded,
   completed row (`journal/mod.rs:1262`). So a receipt whose process never got
   a row is kept forever. The code comment says why: "Unfinished or unrecorded
   Processes retain the only evidence of their exit."
   Evidence: a random sample of 400 archived receipts found 398 with no
   `processes` row by id, receipt locator or trace, and 2 with a row that never
   completed.

3. **Almost all of them come from one 7-hour burst.** 231,887 archived receipts
   are dated 2026-10-16 between 02:39 and 09:01 PDT, at about 35,000 per hour
   (roughly 10 `lf` launches per second), and nearly each has its own trace.
   Only 3,700 processes in that window got a row. Those recorded rows are
   overwhelmingly `lf task file <task>` and `lf task diff <task>`, hundreds per
   Task across about 10 Tasks. So the storm was Desktop's Task Files view
   re-reading files and diffs, and about 98% of those runs failed their ledger
   insert under the storm's own contention, leaving orphan receipts.

4. **The Files view fed itself (leading hypothesis).**
   `TaskFilesModel.invalidate` (`swift/LoopflowMac/TaskFileDocument.swift:403`)
   skips only `.git/` paths. Any other change schedules `refreshChanges()`
   (`lf task diff`) and, for open documents, `lf task file`, 150 ms later.
   `TaskFileObservation` watches the whole checkout with no ignore filter. Every
   `lf` run inside a repository writes a trace under
   `<checkout>/.lf/journal/traces/` (`journal/mod.rs:24`, `:791`). Verified
   2026-10-21: one `lf task diff LOO-538 --files --base parent --json` raised
   LOO-538's trace count from 4 to 5. So each read emits the filesystem event
   that schedules the next read, about one cycle per second per open Task.
   #1559 (2026-10-15 22:53, "Keep files and PRs beside Sessions") shipped Files
   beside Sessions about four hours before the storm began.

5. **Nothing bounded or surfaced it.** Receipts have no age or count cap.
   `lf disk clean` treats unrecorded receipts as evidence to keep. The ledger
   failure is warned once per process and then hidden at debug level
   (`first_ledger_failure`), and nothing aggregates "N Processes unrecorded".
   The storm ran for 7 hours overnight and its debris sat unnoticed for five
   days, until the scan cost became user-visible.

## Contradicting evidence and open questions

- **No storm today.** The loop's preconditions hold on main today, yet the last
  hour shows only 80 `task diff` runs and the receipt count is stable. Possible
  reasons, none verified: no Files tab is retained or open right now; the
  Oct 16 build differed (for example, the Files tab was always observing); or
  something stopped the storm at 09:01 (2026-10-16 08:29 #1570 "Keep Desktop
  clients running during CLI-only updates" is close). Reproduce by opening a
  Task's Files tab and counting `task diff` rows per minute.
- **Lock attribution.** The "database is locked" errors were not traced to a
  holder. About 150 `lf` processes from four builds hold the store open. The
  monitor's read load is a plausible contributor, not a proven one.
- **The Oct 17 04:27 receipt** and the 2026-10-07 and 2026-10-18 strays suggest
  a slow background leak too: killed processes never write completion.
- **Is unrecorded-exit evidence worth keeping at all?** The rule that retains
  it predates this storm. It needs a decision, not just a cap.

## Proposed prevention (not approved)

| Level | Change | Why it addresses the cause | Proof |
|---|---|---|---|
| Desktop | `invalidate` ignores `.lf/journal/` (or every git-ignored path, as the comparison does) | Breaks the read→trace→event loop at its source | View test: a trace write under `.lf/journal/` schedules no refresh |
| CLI | Read-only `lf task file`/`diff` stop writing a repo trace, or traces move out of the checkout | Removes the event even for other watchers | Integration test: `task diff` leaves the checkout's `.lf/` unchanged |
| Store/runtime | Receipts for dead PIDs with no row and older than a grace period are pruned, by `lf disk clean` and the budget rotation | Bounds orphans from any future storm or crash | Unit test over a fixture with recorded, unrecorded and live receipts |
| Monitor | Activity reads receipts incrementally (changed files only) or only those of unfinished rows, instead of a full parse every 2 s | Monitor cost stops growing with history | Benchmark: snapshot time is flat from 1k to 200k receipts |
| Observability | `lf disk` reports the receipt count, and repeated unrecorded Processes surface as one warning with a count | A storm becomes visible in hours, not days | `lf disk --json` fixture includes the count |

The Desktop invalidation fix and the receipt pruning are the highest-leverage
pair. The first removes the trigger; the second bounds the damage from any
trigger.

## Next action

Capture the Desktop invalidation loop and the receipt bound as Tasks, then
confirm the loop by reproduction before fixing it. Jack decides whether
unrecorded-exit receipts are evidence worth keeping.
