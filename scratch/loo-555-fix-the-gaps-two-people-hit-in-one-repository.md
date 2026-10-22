# LOO-555: Fix the gaps two people hit in one repository

## Recovery (2026-10-21)

Jack requested recovery after Claude launches failed writing a Keychain
credential; work continued from the existing commits with account configuration
unchanged. Synced main through `ce21904f` (#1607): kept person keys and
public-name privacy, main's insertion ordering for unstamped ownership entries
and its provider-neutral connection test (the branch writer now uses that API).
The two-person docs and README/index introductions describe Git comment steers,
branch reuse and personal crons. PR copy is in `scratch/pr-title.txt` and
`scratch/pr-body.md`; keep its validation limits and former cron install lines.

Synced main through #1643 (`c0617ec0`, Flow → Graph rename): branch code, docs
and tests now say Graph (`graph:` edges, `.lf/graphs/`, `CronSpec.graph`). Main's
`rename_flow_to_graph` draft rewrites `wave_workflows` content before this
branch's `two_person_gaps` draft drops the table (drafts apply alphabetically),
so its Wave-YAML assertion was removed from the rename upgrade test.

Status: implemented, merged with main through #1643 (`c0617ec0`), committed;
next gate. Seven kickoff passes on 2026-10-21 changed no plan; their narrative
lives in this file's git history. Jack Heart filed the seven
items on 2026-10-21 from LOO-550's two-person docs review. They are accepted direction.
The mechanisms below are kickoff choices; the consequential ones are logged in
`scratch/questions.md` for Jack.

Decision, Jack Heart, 2026-10-21 (878ecf design review steer): LOO-555 lands
first and carries the identity foundation 878ecf (Workflow person nodes,
cross-person handover) builds on.
- The person key is the lowercase Git `user.email`, not the `planning_user`
  UUID, so a committed Workflow file can name a person. Keep the "one of your
  Machines" rule. A missing email is an error at claim time.
- Superseded the same day by two later steers: there is no Machine record or
  Machine publish setting. Crons are private by default and published per cron
  (`lf cron publish/unpublish <name>`, `lf cron add --publish`). Only
  published crons reach Git, each carrying its person key, Machine id and
  Machine label.
- Later the same day: Machines are private by default. A Machine's local label is
  never published; `lf machine public <name>|off` (local setting, default off)
  sets the name ownership entries and published crons carry. A private
  Machine's entries carry its id and no label. Implemented here as
  `machines.public_name` (local row only).

Beneficiary: two people working in one repository, each on their own Machine
(Jack and a teammate, "Dustin", in etude on Git planning).

## Demo

`lf-world build two-people /tmp/pair` builds two Homes: Maya and Lee. They
share one bare `origin.git` with `planning: {provider: git, shared: team}`
committed, and each `env.json` carries its own `LF_HOME`, Git identity and
`user.name`. Then:

```bash
# Maya                                    # Lee
lf task create "GPU fallback"; lf task run lf-3f2a  # → maya/lf-3f2a-gpu-fallback, PR #1
                                          lf refresh                       # Maya's Task appears now
                                          lf comment lf-3f2a "keep CPU path"
# Maya's Session's next tool boundary shows "Comment … by Lee: keep CPU path"
lf wave show                              # owner column: Maya / Lee / —
                                          lf machine public lee-mbp
                                          lf task claim lf-3f2a; lf task run lf-3f2a
                                          # → checks out maya/lf-3f2a-gpu-fallback; finds PR #1
lf task status lf-3f2a                    # Oct 21 14:02  claim  Lee on lee-mbp
                                          # (a private Machine shows its id: "Lee on home_…")
lf machine public maya-mbp
lf cron add nightly --publish -- --wave infra graph telemetry-daily
                                          lf cron list   # Maya · nightly · daily 09:00 · maya-mbp (not installed here)
```

The demo then repeats the claim with `user.name` unset in both Homes. Each person
sees the other's claim as someone else's.

## Items: findings and chosen fixes

### 1. Git-synced comments steer agents

Finding: Git import inserts rows through `planning_write::ensure_record`
(`store/sqlite/planning_write.rs:444-461`). The ownership hook at
`store/sqlite/planning_peers.rs:302-320` runs for every imported comment and
creates no steer. Local comments steer at `store/sqlite/task_comments.rs:227-247`
(source `comment:{id}`, revision `local`, text `Comment {id} by {name}:…`).
Linear ingest steers through `children.rs:707-737` and `ingest_linear_comment`.
The dedup is `UNIQUE(source, revision)`, and `steers::save_in` errors when a
duplicate key carries different text.

Fix:
- `ensure_record` returns whether it inserted.
- Factor the local block into `comment_steer_in(tx, task, &TaskComment)`, filtered
  by `is_direction_comment`. Call it from `append_task_comment` and, for newly
  inserted `task_comments` only, from `import_peer_planning` next to
  `ingest_marker_in`.
- Same rendering means the same `(source, revision)` and the same text, so the
  original Machine (which never re-inserts) and any echo agree.
- Your own comment written on another of your Machines steers that Task's
  Sessions here. This is intended: a comment steers the Task wherever it runs.

Docs: scope-free the homepage line and the README/index opening "a comment on a
Linear task" (both currently exist only in LOO-550's worktree).

### 2. Takeover keeps one branch and one PR

Finding: `plan_task_placement` (`ops/task.rs:1204-1260`) reuses
`tasks.planning_branch_name` when it is set. Only Linear's `branchName` sets it.
`WorkValue::TaskBranch` already travels in the Git journal
(`planning_write.rs:84`) but is never written in Git mode. Checkout reuse already
works: `plan_branch_placement` picks `CheckoutExisting` for `origin/<branch>`,
`restore_task_checkout` creates the worktree from the remote head, and
`branch_pr` finds the open PR by `gh pr list --head`.

Fix: under Git planning, the first placement saves `TaskBranch(Some(branch))`
through the planning writer, in the same transaction as `place_task`. Placing an
already-recorded branch writes nothing. No PR field is added; the branch
identifies the PR.

Under Linear, whether `branchName` is per-viewer is unverified (questions.md);
this Task leaves the Linear path unchanged.

### 3. Crons become personal data

Finding:
- Repository `crons:` (`config.rs:252-268`, four entries in `.lf/config.yaml`)
  feeds `cron_specs` → `lf cron sync`. Anyone who syncs installs every entry.
- The installed truth is already personal: launchd plists
  `loopflow.cron.<wave>.<flow>` carrying command/schedule metadata
  (`ops/cron.rs:1314-1337`). `lf cron add` stores nothing else.
- Machines are not exchanged as records. Teammates see a Machine only as the label
  in ownership comments.
- Neither Linear nor the Git journal has a person- or Machine-level record. Note
  that loopflow itself plans on Linear.

Mechanism (Jack's steers): crons are private by default. A Machine pushes the
crons it publishes for this repository to its own ref,
`refs/loopflow/crons/<machine-id>` on `origin` (`ops/cron/published.rs`).
- The ref's commit holds `crons.json`: `[{name, wave, command, schedule, person,
  person_key, machine_id, machine_label}]`. With nothing published, the ref is
  deleted.
- The published flag lives in the cron's plist (`LoopflowPublished`).
- There is exactly one writer, so a plain force push is safe, with no merge logic.
- It is independent of the planning transport, so it works for loopflow (Linear)
  and etude (Git).
- `lf cron add/remove/publish/unpublish/sync` push. `lf cron list` fetches
  (`--prune`) and shows yours, then teammates' published crons with person and
  Machine. `--json` is `{installed, published, fetch_error}`.
- When the fetch fails, the listing still shows the last observed state, labeled
  with its age.
- Running is unchanged: only the Machine that installed a cron runs it.

Reshape:
- `CronCommand::Sync` without `--repo` reinstalls this Machine's plists with the
  current lf path and publishes. `--repo` (Desktop's automation job) is unchanged.
- `Preflight` validated the config list and is deleted.

Migration: Jack's laptop already has `infrastructure.release-run` and
`infrastructure.telemetry-daily` installed. After upgrade, `lf cron sync` there
publishes them. The repository's other two entries (intelligence
`telemetry-daily`, product `wave/operate`) are not installed on this laptop. The
PR body lists the exact `lf cron add` lines for whoever wants them
(questions.md):

```bash
lf cron add telemetry-daily --schedule "0 0 8 * * * *" -- --wave intelligence --batch graph telemetry-daily
lf cron add wave/operate --schedule "0 0 8 * * * *" -- --wave product --batch graph wave/operate
```

Installed crons are private after upgrade; `lf cron publish <name>` shows one
to teammates.

### 4. Owners wherever Tasks are listed

Finding: owner data (`runtime.owner: TaskOwner{person, machine}`) already
reaches every list's JSON (`lf/commands/waves.rs:165,1287`; Swift
`WaveWorkMap.swift:146`). The text surfaces drop it:
- In `lf wave show` (`print_roadmap`, `waves.rs:2349`), the third column is
  `next_move.owner`.
- `lf task list` (`waves.rs:766`) and the `lf wave status` Task line (`:1922`)
  have no owner.
- Desktop's Wave inventory (`WorkSurfaceView.swift:27-55`) groups Owned/Unowned
  without names. `WaveDetailPane.swift:710` shows only "next: wave".

The log lines print `at` as epoch seconds and `machine_id`
(`ops/task_ownership.rs:98-120`). A teammate's Machine id has no local label,
although the claim comment's prose carries one.

Fix:
- `lf wave show` renames the column `next` and adds `owner`: person name, or `—`
  when unowned; "Someone" when the name is unknown. The same owner text goes into
  the `lf task list` and `lf wave status` Task lines.
- Desktop shows `owner.summary` on Owned-group inventory rows and in the Wave
  detail Task line.
- The ownership entry and its marker gain `machine_label`: the claimant
  Machine's public name when written, or none when it is private. `ownership_in` resolves each log line to a `TaskOwner` (local
  label, then registered label, then stored label).
- Log lines read `Oct 21 14:02  claim  Lee on lee-mbp`. They use one shared
  `format_time` in `lf/output.rs`, replacing the identical copies in
  `context_usage.rs:654` and `lf/commands/usage.rs:132`.

### 5. `lf refresh` works for Git planning

Finding: `lf repo refresh` always runs `ops::linear::refresh`. Under Git it fails
on "no repository `linear.team`" or on a missing Linear credential.
`planning_sync::acquire_repository` is already the single "fetch and import now"
call, and it is a no-op for non-Git transports.

Fix:
- `sync_planning` branches on `planning_transport()`. Git runs
  `acquire_repository` on the main repo root, then prints the destination line
  `lf planning status` already renders.
- An acquire failure exits non-zero with the recorded error.
- `[WAVE]`/`--all` are accepted and have no effect under Git: the exchange is
  per repository, and the help says so.
- `lf wave show` stays effect-free; a connected Session still polls every 5s.

### 6. Ownership compares a stable person identity

Finding:
- `is_mine` (`store/sqlite/task_ownership.rs:199-203`) compares the display name
  to `participant_name()`. A missing name on either side reads as yours.
- The log has no identity column. The only stable per-person value is the random
  `planning_user` key (`store/sqlite/planning_peers.rs:79-110`). That key is
  provisioned lazily for personal Git refs and carried to your other Machines
  with `lf planning key --recover`.

Fix (person key per Jack's steer):
- Your person key is the lowercase Git `user.email` in the Task's repository
  (global when that checkout is absent; `config::participant_email`).
  A claim without one fails, naming `git config --global user.email`. Handoff
  and reassert carry the holder's key; unclaim records yours when set.
- Ownership entries and markers gain `person_key` and `machine_label`; the log's
  `machine_label` is resolved for display (local, registered, then written).
- `is_mine_in(holder)`: a keyed holder is you exactly when the key matches
  (a teammate's claim on a Machine you registered stays theirs); a keyless
  holder is you when its Machine is one of yours (`is_my_machine_in`: local,
  or added with `lf machine add`).
- Names are display only. A keyless entry on an unknown Machine is someone
  else's. Entries recorded before ownership existed on this Machine stay its
  own through the Machine rule.
- `lf --machine` claims carry the remote's `user.email`; use the same email on
  all your Machines.

One person on two Machines with the same `user.email` reads as one person
without setup. CI sets a fallback global identity in its test jobs; repository
-local test identities win.

### 7. Wave Workflow `--file` definitions

Verified true:
- `select_wave_workflow` (`store/sqlite/project_content.rs:78-110`) journals only
  the name. The content stays in Machine-local `wave_workflows`.
- Linear carries neither the name nor the content.
- A teammate fails with `wave Workflow "x" is unavailable`, or silently runs a
  same-named builtin or file. The sender's snapshot also hides later commits to
  `.lf/workflows/`, because import uses `ON CONFLICT DO NOTHING`.

Live data (read-only, `~/.lf/loopflow.db`): four rows, all loopflow.
- Intelligence selects `iterate`, which exists only there and references a `build`
  Flow that resolves nowhere. It is already unrunnable.
- `state-spring-cleaning` is unselected.
- `autonomous` matches `.lf/workflows/autonomous.yaml`.
- `code` matches the builtin.

Fix: definitions live only in `.lf/workflows/<name>.yaml` (committed, so they
travel with the code) or builtins. A Wave selects a name.
- Delete `--file`, `list --wave` and the `wave_workflows` table.
- `lf wave workflow set` keeps its existing check that the name resolves.
- The unavailable error names the missing `.lf/workflows/<name>.yaml`. It may not
  be merged yet.

## Architecture before → after

| Concern | Before | After |
|---|---|---|
| Comment steer | local + Linear ingest, two inline renderers | one `comment_steer_in`; local, Linear and Git import |
| Task branch under Git | derived per runner | first placement journals `TaskBranch`; every runner reuses it |
| Cron definitions | repo `crons:` + launchd plists | launchd plists (local truth) + per-Machine published ref |
| Owner display | JSON only; one Desktop row | every CLI list and Desktop Task line; readable log |
| Refresh | Linear only | per transport |
| Ownership identity | display-name equality, missing = mine | person key or own Machine |
| Wave Workflow definition | SQLite snapshot + repo files + builtins | repo files + builtins |

Source of truth:
- Planning: the SQLite planning store plus its transport.
- Crons: this Machine's launchd plists. Teammates' crons are a fetched,
  read-only view.
- Workflow definitions: repository files.

Schema: one draft (`uv run python scripts/new_migration.py two_person_gaps`):
- `task_ownership` adds `person_key TEXT` and `machine_label TEXT`
  (append-only trigger recreated with both).
- `machines` adds `public_name TEXT`.
- Drop `wave_workflows` and its revision triggers.

Wire DTOs: `TaskOwnershipEntry` (+`person_key`, `machine_label`) and the log
item type in `task_status.json`. Each language's fixture test is updated. Swift
mirrors only what it reads (`runtime.owner` is unchanged).

## Delete — do not maintain

- `Config.crons`, `CronDef` and the `crons:` block in `.lf/config.yaml`.
- `cron_specs`, `CronCommand::Preflight`, and the config-driven
  `Sync{wave,task}` path.
- `cron_catalog_tests` (`lf/commands/ops/mod.rs:1606-1665`) and
  `python/tests/test_release_automation.py:193-205`.
- Stale cron mentions:
  - `builtins/task/skill/prompt.md:171`
  - `builtins/ops/skill/init.md:291`
  - the GOAL.md-frontmatter crons in `scripts/bootstrap-cron-host.sh:96` and
    `release/CRON_HOST.md:51`
- `wave_workflows` and everything that reads or writes it:
  - `project_content.rs:50-68` and `:101-107`
  - `wave_definitions.rs:84-125` and its callers (`:35`, `:76`, `sqlite.rs:828`,
    `migrations.rs:816-817`)
  - the `sqlite.rs:2002-2011` blocker
  - `project.rs:262-285` ("stored") and `:417-425`
  - `WaveWorkflowCommand::Set.file` and `List.wave`
  - the tests in `project_content.rs:357-433`, `planning_write.rs:783-800`,
    `task_planning_tests.rs:640-670`, `wave_definitions.rs:245-265` and
    `tests/linear/wave_repository_ownership.rs:823` that exercise stored
    definitions
- The name branch of `is_mine`.
- The duplicate `format_time` in `context_usage.rs` and `usage.rs`.

Forbidden:
- Reading repository `crons:` as a fallback.
- Dual cron sources.
- A Wave-definition exchange alongside the files.
- Comparing names "when no key exists".
- A second steer renderer for Git.

## Exclusions

- Default shared Git ref (LOO-552).
- Operator "theirs" disposition and the operator-focus Task.
- Linear per-viewer branch names.
- A Desktop cron view.
- Reviewer-named Workflow nodes.
- Publishing Machines themselves.

## Slices (one PR, ordered deletions first)

1. Wave Workflow files only (item 7): migration draft drop, deletions, error
   text, docs.
2. Crons (item 3): delete the repo key and paths; per-Machine ref publish/list;
   the `lf-world` cron fixture uses a bare origin.
3. Identity and display (items 6, 4): migration columns, `is_mine`, marker
   fields, log rendering, `wave show`/`task list`/`wave status`, Swift rows.
4. Comment steers (item 1), branch journaling (item 2), refresh (item 5).
5. `two-people` lf-world recipe and the `tests/two_people.rs` acceptance suite
   (registered in `Cargo.toml` if standalone). Docs: `docs/lf.md` "Work with
   another person" (from LOO-550), README/index opening, homepage, `docs/config.md`
   crons, `docs/authoring.md`/`waves.md` Workflow definitions, regenerated
   `docs/lf-reference.md`.

All five slices are implemented.

Main is merged through #1607 (`e22a86bf` + repairs `c2b94445`, `575d15c8`).
The merge kept this branch's two-person docs over LOO-550's caveats, main's
deletion of `peerPlanningStatus` and `world_recipes.rs`, the six CI fallback
Git identity steps, and `planning_client::connected` (main's provider-neutral
test, covering Asana and Notion, which steer through Linear's ingest path).
`two_people` runs in ~28 s, inside nextest's `pr` 120 s limit.

## Done when (gate)

- `cargo test -p loopflow --test two_people`: on one built `two-people` world,
  each item holds:
  1. Lee's Git-synced comment is pending in Maya's Task Session steers
     (`lf session steers`).
  2. Lee's `task run` reports Maya's branch and the same PR number (world `gh`).
  3. Neither Home installs the other's cron. `lf cron list --json` on Lee shows
     Maya's cron with person, Machine id, and Machine label only when
     Maya's Machine has a public name.
  4. `lf wave show` has an owner column with both names. `lf task status` log
     lines contain a formatted date and Machine label, with no 10-digit epoch.
  5. Lee's `lf refresh` imports Maya's new Task, exit 0.
  6. With no names set, Lee's `lf task unclaim` on Maya's claim fails "claimed by
     someone else", and Maya's own `unclaim` succeeds.
  7. `lf wave workflow set --file` is rejected by Clap. A Workflow committed to
     `.lf/workflows/` by Maya runs as Lee's Wave default after `git pull`.
- Affected suites: `cargo test -p loopflow` (planning, ownership, cron, waves),
  `scripts/test_desktop.sh --filter 'WaveWorkMap|WorkSurface|WaveDetail'`,
  `uv run pytest python/tests/test_release_automation.py`, `cargo clippy
  --all-targets -- -D warnings`, `documented_commands`. All pass.
- Docs: no page claims comments steer only under Linear, or that crons live in
  `.lf/config.yaml`.

## Concept review 2026-10-21: published ownership prose

Done: ownership comment prose (`describe`, `store/sqlite/task_ownership.rs`)
names only what the entry carries — its public name, else the Machine id —
so neither the hostname nor an `lf machine add` label is published.
`machine_ref_in` stays display-only (`ownership_in`, `resolved_in`, log lines).

Done (second pass, same day): `lf machine public <name>|off` saves the
setting, then republishes every published cron once per distinct installed
`cron.repo` (`republish_crons`, `lf/commands/ops/mod.rs`), so `off` removes
the public name from Git without a later cron command. `two_people` item 3
asserts Lee's listing shows a null `machine_label` right after `off`.
Already-published ownership entries are append-only history and keep the
name they were written with.

Owner display (third pass, same day, done): an owner with no `user.name`
displays its person key, since names are display-only and "Someone" hid the
one value that tells Maya's claim from Lee's. `TaskOwner.person_key` (from the
holder) and `durable::person_text` give name, then key, then "Someone" in
`TaskOwner` Display, `owner_text`, published `describe`, ownership log lines
and the `lf cron list` person column; Desktop's `TaskOwner.summary` matches.
`lf wave show` sizes its owner column to the widest owner (12–32) so an email
is not truncated. DTO fixtures carry `owner.person_key` as a lowercase email.
The key is already published in every marker and published cron, so showing
it exposes nothing new.

## Progress

All seven items are implemented, including the 2026-10-21 steers: crons are
published per cron, there is no Machine record, and Machines have a private
label plus an optional public name. `tests/two_people.rs` proves all seven
items on a built `two-people` world. `lf refresh` under Git is a full
exchange: it acquires, then publishes pending changes, so one command
converges both people.

The World's `bin` now stands in for `launchctl` (records jobs under
`launchd/`), and its `gh` answers `--version` and `pr list --head`. Item 3's
check writes Maya's private cron with `ops::add_cron`, because `lf cron add`
refuses non-release builds. It then runs the real `lf cron publish`, before
and after `lf machine public`. `World::person` also names the World's Home,
since the Home's `user.name` outranks Git's. Item 2 is proven by Lee's real
`lf task claim` + `lf task run <task> takeover` (an agent-free Workflow),
then `task status --json` shows Maya's `maya/…` branch and PR #1 from the
World's `gh`. Item 3 asserts Lee's `cron list --json` has empty `installed`,
and Lee's Home has no LaunchAgents.

Remaining (gate): full `cargo test -p loopflow` (lib + integration),
`documented_commands`, the `cli_reference` check (`docs/lf-reference.md` already
lists `lf machine public` and `cron publish/unpublish`), and the PR body's
`lf cron add` lines for the dropped intelligence/product repo crons.

## Check results

- 2026-10-21 realign (after the #1643 Graph merge): demo and PR-body cron lines say `graph`; fmt clean, lib `published` 17, `two_people` 1 passed. Gate owns the full suite.
- 2026-10-21 compress (refresh split into `plans_through_git` + `refresh`, `published_from` as filter/map, cron-host docs say release jobs stay private, init skill's stale cron line): lib `published` 17 passed, `two_people` 1 passed, fmt + clippy clean. Every Delete-list item is done.
- 2026-10-21 implement re-pass: no code change needed; lib `task_ownership` 8 passed (incl. both private-Machine publication tests), fmt clean. Realign (after `lf machine public` republish): `two_people` 1 passed (80s), lib `published` 17 passed, fmt + clippy clean; earlier same day lib `task_ownership` 8, `cron::` 47, `world_recipes` 3, `test_desktop.sh --filter DesktopHeadless` 14, `pytest test_release_automation.py` 10 passed, clippy clean. Gate owns the full suite.
- 2026-10-21 implement (owner key fallback): build ok; lib `task_ownership` 8, `two_people` 1, integration `dto` 42, swift `TaskRowMetadataTests|Fixture` 77 passed; fmt + clippy clean. Gate owns the full suite.
- 2026-10-21 compress (second pass): the store's private `Holder` is gone; `TaskOwnershipEntry::owner()` yields the `TaskOwner` the reading, the published prose and the log renderer all use, with `resolved_in` adding the best-known Machine label. fmt + clippy clean; lib `task_ownership` 8 passed, `two_people` 1 passed. Gate owns the full suite.
- 2026-10-21 kickoffs (six relaunch passes, latest after the six-hour sweep, work committed as `5b725154`): each pass `cargo fmt --check` clean, `cargo check --all-targets` ok (~27s), `two_people` 1 passed (19–30s); the third also ran integration `reference_matches_the_compiled_cli` 1, `documented_commands` 4, `dto` 42 and lib `task_ownership published cron:: planning_git planning_peers task_comments` 77 passed (one `task_comments_read_and_publish_without_starting_work` failure under a 117s loaded batch, an untouched Linear HTTP fixture, passed on three reruns). The fifth fixed `ci.yml` `swift-test` (a `run:` step had split the checkout from its `with:`). No plan change. Gate owns the full suite.
- 2026-10-21 seventh kickoff (after the #1607 merge): main's new `unstamped_entries_in_one_second_keep_their_written_order` applied released migrations only and missed the draft's `person_key`; it now opens the store fixture. fmt + clippy clean; `two_people` 1, lib `task_ownership` 9, `published` 17, `cron::` 47, `planning_peers` 2, `task_comments` 4, integration `reference_matches_the_compiled_cli` 1, `documented_commands` 4, `dto` 42 passed. Gate owns the full suite.
- 2026-10-21 recovery: architecture, `pytest test_release_automation.py` 10, website 72, README/index consistency, migration validation, fmt + clippy passed; headless Desktop 613 tests ran with one sidebar frame race, now waited on (`test_desktop.sh --filter WorkCLISubscriptionTests` 3 passed). Full Rust suite was stopped by the gate's macOS security-pressure monitor before running; CI owns it.
- 2026-10-21 implement re-pass: no code change since `9cc03ed6`; scratch trimmed of ambient facts, no rerun.
- 2026-10-21 compress (after recovery): `PublishedCron` embeds `Publisher` (flattened, same `crons.json`/`--json` shape) instead of copying its four fields. clippy clean; lib `published` 17, `two_people` 1 passed. Gate owns the full suite.
