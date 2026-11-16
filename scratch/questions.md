# LOO-925 questions

## Taken headless (reversible at review)

- **Any tracker, not Linear-only.** Linear and Asana stay as bridges; Notion is
  deleted. Basis in `tracker-mirror.md` ("Decision"): the 2026-11-02 parity
  decision recorded on `loo-691-combined`, Asana's rebuild in #1607, and the
  queue being required either way. If Linear-only is wanted, slice 4 deletes
  `asana/` instead of porting it and everything else stands.
- **Table name `tracker_effects`.** Alternatives: `planning_outbox`,
  `tracker_queue`.
- **Drain bound: 20 provider requests or 5 s per pass, concurrency 4.** The
  Linear allowance is 5,000/h shared by every lf process on a Machine (~11 live
  on 2026-11-05); 20 per pass keeps a pass well under it. Tune from
  `linear-requests.jsonl` after landing.
- **Migration marks copied old rows settled** so an older lf does not resend
  them; edits an older lf makes after install are delivered by that lf.
- **`lf task sweep` is deleted** with `ChapterSweep`: it sweeps Tasks outside
  the "current chapter", a rotation `planning.md` says no longer exists. If a
  sweep of Tasks outside the Wave's current Project is still wanted, it is a
  new command over Projects, not this one.
- **`~/.lf/chapter-locks/` keeps its name** until the contract Task: an older
  lf on the same store locks the same inode.

- **"Chapter" names stay where they name the current Project**
  (`chapter_metric_portfolio`, `MetricContractIssueDto::ChapterUnavailable`,
  "unset for this chapter", `wave new-chapter`). AGENTS.md defines Chapter as
  the In Progress Project's shared name, and `chapter_unavailable` is a wire
  tag Swift decodes. Slice 1 deleted the mechanisms (sweep, adoption), not the
  word.
- **GOAL.md name/summary no longer reach the Initiative.** The only writer was
  `refresh_async`'s apply branch, reachable only from a test; it and
  `LinearSyncOptions.plan` are gone. Wave `field` effects (slice 3) restore it
  with a receipt.

- **Losing value after a conflict shows in `lf task show`** (`sync.conflict`),
  the surface the Demo reads; Desktop follows from the DTO. Alternatives were
  `lf repo plan` or Desktop first.
- **Effects only for bound owners or a pending create.** Checked: the old
  export drain exports every Task with no `external_issue_id` once a tracker
  is configured, including Tasks made before connect. Slice 3 keeps that with
  a connect-time `create` enqueue at the start of a drain pass.

- **Effect ids are fresh UUIDs.** Bridges derive idempotency keys from the
  owner (a Task's uuid is its Linear issue id, as today) and the subject
  (comment id), so a retired and re-queued `create` cannot collide on `id`.
- **Copied rows are `provider='linear'`.** Released rows never recorded their
  bridge; every row on this Machine is Linear. An Asana store's copied effects
  would retire with "queued for Linear" on the first pass.
- **Copied comment deliveries start `attempted`.** The old table has no attempt
  flag; Linear resends with the comment id and Asana matches visible text.
- **A pending `order` looks before it sends** (one request): moves need the
  provider's complete list.

## Open

- Connecting a tracker to a repository with existing Tasks: export all of
  them (today's behavior, planned) or only Tasks made after connect? The
  second would make the connect-time enqueue unnecessary, but it drops
  behavior people may rely on.

- Wire `linear_id` → a neutral name crosses Rust, Swift and ~10 DTO fixtures.
  Planned as a follow-up Task, not here. Is a neutral name wanted at all while
  Linear is the only tracker in use?
