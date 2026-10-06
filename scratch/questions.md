# Open questions and assumptions (LOO-382)

Questions resolved or overtaken before the second round are at commit
`018083656`; their numbers are kept so older notes still resolve.

## Decided by Jack Heart, 2026-10-05

- **3, 30.** Desktop reacts to the store only; Linear arrives through sync.
  Nothing schedules that sync for Desktop. Jack prefers webhooks; no Task.
- **4.** The latency budgets are targets pursued for about an hour; a miss
  ships with measured numbers.
- **8.** One landing.
- **22, 29.** A Task that has not started stays out of the outline for now.
  LOO-382's first acceptance line is not met as written.
- **15, 30.** No follow-up Tasks.
- **31.** Questions 5, 9, 12, 13, 17, 18 and 26 became the second round.

## Unconfirmed

1. **Store revisions are a shared-store change.** One draft migration adds
   `store_revisions`, its triggers and three indexes (`execs_unfinished`,
   `session_events_exec`, `flow_events_exec`). The second round added a fifth
   domain, `usage`. Infrastructure has not seen any of it.
2. **Read costs outside the planning reading have no owning Task** (LOO-375
   owns `wt list` only).
6. **Per-read Exec recording.** Whether a one-shot read should record an
   Exec at all is Infrastructure's. Slice 11 was dropped on it (question 37).
7. **Desktop's Create uses `task create --run`.** Whether it should offer
   create-without-run is a product choice outside this plan.
10. **The transcript predicate** exempts only the types the summary readers
    skip. `session list` fields are tested; Work activity and Wave history
    are not.
14. **Process activity is read every 2 s inside the watch.** Measured in the
    second round: the reader left alone uses 1.6% of a core and 37 MB on the
    copy, over the 1% target. Not profiled.
16. **Session fixtures read once.** UI-test modes read Sessions at launch and
    after local actions. No UI test was run.
20. **The write-to-visible benchmark writes rows with `sqlite3`**, because
    `lf task create` needs Linear. It measures the reader and the window, not
    `lf`'s own write path.
21. **A frame from another Home is applied after dropping the old Home's
    content**, where the plan said it would be ignored.
23, 32. **Derived state in the database.** Jack said "i think we want it in
    the db", then that SQLite reads should not be a major problem. Taken as:
    nothing derived is stored while reads meet their budget. Not confirmed
    in those words.
24. **A failing part backs off** 1 s doubling to 60 s. The cap is a guess.
25, 33. **The planning reading still runs statements per unfinished Task**
    (about 40 ms of 200 ms). Left alone as not a clear win; Jack did not
    follow the question as asked.
27. **`unresolved_execution` is a required field.** Workspace text saved by
    an earlier build should show no saved planning until the reader answers.
    Not exercised.
28. **The outline's "has open Sessions" and "started" tests stay in Swift.**

## Added in the second round, 2026-10-05. Nobody has confirmed these.

35. **Slice 10 dropped: `active` stays its own process.** Jack listed it
    among the taboos to remove. The measurement (0.2% of a core, 25 MB,
    started only on Monitor demand) is in the plan; the copy has no `runs/`
    receipts, so the real idle cost is higher. If one process per window
    matters for a reason other than cost, this is his to reverse.
36. **A checkout an agent builds in is asked about every 3 s** (four Git
    commands), and a change shows in about 2 s. Both numbers are guesses
    that measured well once. CPU of the watch during a build is unmeasured.
37. **Slice 11 dropped: every `lf` command still re-runs planning in the
    reader**, twice, without a frame. Exempting a read needs the store to
    know a command only reads. That is the same change as question 6.
38. **The watch's Git reads set `GIT_OPTIONAL_LOCKS=0`**, so the reader never
    takes a checkout's index lock from an agent. One-shot commands are
    unchanged.
39. **Token totals may be 10 s old**, and only Wave detail follows usage.
    Whether another surface shows a total that still lags was not surveyed.
40. **Slice 9 watches in Swift, slice 7 in Rust.** Two watchers of the same
    two directories per shown Task, each feeding its own reader. Folding the
    comparison into the stream as a part would remove one and the `lf` spawn
    per change. Not built.
41. **The first reading asks Git about eight checkouts at once.** The number
    is a guess; it was not varied.
42. **FSEvents paths on a temporary volume never match the Swift checkout
    root** (`/private` prefix). Left alone: real checkouts are unaffected
    and open documents are covered by vnode sources.
