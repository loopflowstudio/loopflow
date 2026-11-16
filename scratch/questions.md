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

- **`conflict_json` holds the provider's winning `{revision, value}`**; the
  effect's `value_json` is the local value that lost. Status shows both.
- **Head-of-line per subject, not per owner**, except that everything on an
  owner waits behind its unsettled `create`. Old state and comment delivery
  were independent; a stuck state write no longer blocks a comment.
- **Edits to an owner with neither a tracker object nor a queued `create` are
  not queued**: the creation reads current values when it is queued.
- **A historical filing's `create` keeps store words only**; its original
  Team and state ids are not retained.
- **`lf task sync` looks now; `--retry-updates` also ignores the look window**
  (`Bound { windows: !retry_updates }`).
- **Linear is idempotent only for comments.** A Linear create with an existing
  id is an error, so a lost creation reply is looked for, never resent.
- **Order sends one confirmed move at a time** within one send and rebases the
  effect after each move.

- **Links coalesce.** A newer unsent `link` on the same subject retires the
  older one: a PR's Open then Merged sends only Merged. Relations have unique
  subjects, so nothing changes for them.
- **A PR's managed Linear comment id** is the one an earlier lf recorded on the
  PR, else uuid-v5(Task, PR URL). New PRs no longer record attachment or
  comment ids; the attachment is found by URL.
- **Asana follow-up relations retire `Unsupported`**; the follow-up stays the
  comment `recover_task_filings` writes.
- **`pm_issue_changes` stays** as acquisition evidence (released stores hold
  removals it alone records).

## Open

- Connecting a tracker to a repository with existing Tasks: export all of
  them (today's behavior, planned) or only Tasks made after connect? The
  second would make the connect-time enqueue unnecessary, but it drops
  behavior people may rely on.

- Wire `linear_id` → a neutral name crosses Rust, Swift and ~10 DTO fixtures.
  Filed as Task 9fbce4, which opens with this question: is a neutral name
  wanted at all while Linear is the only tracker in use?
