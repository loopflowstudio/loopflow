# LOO-925 questions

## Taken headless (reversible at review)

- **Any tracker, not Linear-only.** Linear and Asana stay as bridges; Notion is
  deleted. Jack Heart, 2026-11-16: "lets keep asana but dropping notion is fine".
- **Table name `tracker_actions`** (was `tracker_effects` before the
  vocabulary pass). Alternatives: `planning_outbox`, `tracker_queue`.
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
  `LinearSyncOptions.plan` are gone. Restoring it as Wave `field` effects is
  new behavior for a follow-up Task, not this one.

- **Losing value after a conflict shows in `lf task show`** as a
  `conflicted` row in `sync.actions` (see concept review round below);
  Desktop follows from the DTO.
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
- **`lf task sync` always looks now** and resends what the tracker lacks;
  `--retry-updates` is gone (concept review 4a).
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
  Filed as LOO-942, which opens with this question: is a neutral name
  wanted at all while Linear is the only tracker in use?

## Vocabulary (2026-11-16, at demo)

Jack Heart: "i feel like there a re a lot of overworking words arounf work,
tracker, plan, etc can we standardize and simplify".


Jack Heart's answers, word for word:
- Scope: "This PR's words; Task for rest" (chosen option).
- Inbound name: "event".
- Outbound: "event inbound, action or update outbound?" Agent recommended
  action (update collides with Task updates, `lf wave update`, update-plan).
- "git is a tracker (work tracker?)". Agent recommended plain "tracker".

Built (glossary in `docs/dev/planning.md` "Words"; renames in
`tracker-mirror.md` "Vocabulary pass"). Calls taken headless:
- "tracker" over "work tracker"; phases `queued · sent · confirmed ·
  conflicted · retired`, with `conflicted` a phase of its own rather than
  `confirmed` plus `conflict_json`.
- The generic verb for leaving the queue is "resolve" (`Resolution`,
  `Decision::Resolve`); "unsettled" became "unconfirmed".
- `TrackerObservation` and `rules::Observed` keep their names: each is a
  tracker's current copy, not one incoming change, so "event" would misname
  them. `ops/tracker_observe.rs` became `ops/tracker_events.rs`.
- Rename scope is lines this PR added; older prose ("settled", "pending
  effects", observation/acquisition) is LOO-955.

- QA 2: a Project `create` the tracker rejects before any request (Wave with
  no Linear Initiative) still takes one of the pass's 20 requests, every pass.
  One such Project on this Machine (manabot `infrastructure`). Concept
  review 2026-11-17, headless: charge only requests that leave, since the
  bound is the provider's allowance. Built as `Tracker::blocked`, decided
  before the drain takes a request or marks the action sent; a missing
  Project or Wave is an error from the send, not a block.

## Concept review round (2026-11-17)

Page: `scratch/concept-review.html`. Jack Heart: "ok, go in for antoher round
of action". Its five questions were not answered one by one; this round builds
these defaults, reversible at review:
- 1a: `task.sync` becomes the Task's slice of `lf repo plan`'s actions (one
  view: seq, owner, change, subject, phase queued|sent|conflicted, tracker,
  error, local_value, tracker_value). Delete PlanningSyncChange,
  PlanningSyncState, `lines()` labels, `linear_value`; Swift
  `PlanningSync.swift`, `planning_sync.json` fixture and Desktop headless test
  follow.
- 2a: keep "action" (Jack's choice); prose says "tracker action" where Desktop
  actions are near.
- 3a: Git is a tracker on surfaces: `lf task sync` on Git exchanges with the
  ref instead of refusing; Git journal losing values show as `conflicted` in
  the same view. No Git queue.
- 4a: drop `--retry-updates`; explicit `lf task sync` always looks now and
  resends what the tracker lacks.
- 5a: already filed as LOO-956.
- Also: `QueuedAction.phase` doc, `transport`/`provider` -> `tracker` in
  PlanStatus/QueuedAction, `tracker::configured()` named for what it returns.

Built, calls taken headless:
- One row type, `TrackerActionStatus` (storage `planning_sync.rs`), in
  `lf repo plan --json` `actions` and in `sync.actions` (`TrackerSync`);
  Swift `TrackerSync.swift`, `TrackerSyncView`, fixture `tracker_sync.json`.
  `connected` left the wire: with no tracker configured, queued rows are
  simply absent. `lf repo plan` now lists conflicted actions too.
- The row has no owner display name (a Task's short identifier costs a scan
  of every Task per read); `lf repo plan`'s text resolves names. `--json`
  consumers get `owner_kind` + `owner_id`.
- Git conflicts are a field's competing journal heads other than the winner,
  `tracker: "git"`, `seq: null`; the loser is `local_value` even when another
  Machine wrote it (the journal does not say whose a head is).
- Git conflicts show in a Task's or Project's `sync` (and `lf task sync`),
  not in `lf repo plan`, which on Git lists destinations. A repo-wide list
  needs every record's journal heads per read; open if wanted.
- One line rendering for CLI and Desktop: `title · conflicted · lost:
  "A" · linear: "B"`. `lf task sync` prints these, else `synchronized`.

Jack Heart's answers to the page, word for word: "1a 2a 3a for now, possibly
a follow up task? 4a 5a". The defaults being built match. Git on the queue
(3c) is already LOO-938's peer half ("Send every planning change through one
queue": planning_peer_changes/observed/routes); no new Task filed.
