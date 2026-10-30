# LOO-694: open a Task's Session after Desktop restarts while its conversation runs

Status: implemented (2026-10-30), realigned against origin/main (no upstream changes to the touched files); awaiting gate.

## Observed failure (2026-10-30, Jack Heart's machine)

- MTG-214's Session `7d29bee8…` runs in owner `2b6fdd25…` (pid 20629), a 0.15.0
  `lf session serve` whose app bundle was moved aside when 0.15.1 installed.
- No viewer process is attached (no `lf session resume 7d29…` exists), yet
  `processes.viewed=1` for that owner.
- `lf session resume 7d29… --prepare --json` (installed 0.15.1) returns
  `state: active`, Open unavailable ("active in another terminal"), Move here
  offered. Desktop's `adoptPreparation(.attach)` maps `active` to `.elsewhere`, so
  showing the Task's pane never reattaches.
- Cause: only the owner writes its `viewed` column. A 0.15.0 owner cannot write
  a store 0.15.1 advanced (lf.log: `detached Session policy stopped …
  incompatible Loopflow database`), so the record outlives the viewer that left
  when Desktop quit. 0.15.1's attach still speaks 0.15.0's protocol (serde ignores
  the added `moved` field; `controlled` exists in both).

## Change

- `human_session::viewed_summary` — one Session's summary with a `viewed`
  record confirmed by the owner's `controlled` reply. `surface()` (every
  `--prepare`/resume record) and `lf open`'s `elsewhere()` share it; `elsewhere`
  had its own probe.
- `transport/lifetime.rs` — a failed viewer write logs and keeps the executable
  and detached-limit policy running instead of ending it.
- Test: `transport_tests::opening_reattaches_an_owner_whose_viewer_record_went_stale`
  (viewer leaves, the owner's record is reset to viewed as a stale owner
  leaves it, prepare reads `detached`/Reconnect, `lf session resume` reattaches the same execution).

## Delete — do not maintain

- `lf open` `elsewhere()`'s private `controlled` probe (folded into `viewed_summary`).

## Remaining

- Passive list (`lf desktop sessions`, `listed`) still reads the column; the
  sidebar may show the stale row as active until the pane is shown. See questions.
- MTG-214's live owner is untouched; opening it with this build reattaches.

## Concept review (2026-10-30, headless)

- Concept unchanged: "viewed" means a controlling viewer holds the owner, and
  the owner's socket answers it; `processes.viewed` is that owner's record of it,
  trusted only where no one-Session decision rides on it. Open/Move here/Reconnect
  keep their meanings.
- A hung or unreachable owner still reads `active`; Move here takes the
  conversation over on the same provider thread, so no Session stays unopenable.
- The passive sidebar label (questions.md) is presentation, not product direction:
  showing the pane decides and reattaches. No open product choice; no change.

## Proposed Wave memory (infrastructure; not written outside a Graph's memory step)

- An owner from an earlier lf keeps running after an upgrade but cannot write a
  store the newer lf advanced. Columns only that owner writes (`processes.viewed`)
  go stale; the owner's socket (`controlled`) is the live truth, and a failed
  owner write must not end its policy.

Check: realign ran no tests (plan and diff agree); gate owns `cargo test -p loopflow --test transport_tests opening_reattaches`.
