# v0.14.18

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.18 reorganizes `lf` around the work itself. Root help now starts with Tasks, Waves, Sessions, Graphs and Skills. Delivery, machine and account plumbing moves under `lf ops`, and the familiar short forms (`lf land`, `lf wt create`, `lf machine use`, `lf release run`) still resolve. The release also fixes things that interrupted work that was otherwise going fine. Headless Claude steps no longer end while their background commands are still running. CI repair and sync can run while you keep working in a Task. Launches go to the account with the most room. Task and Wave status can read Graphs recorded before v0.14.17 again. There is no store format change, so older `lf` versions keep working against the same `~/.lf`.

## A command tree that starts with the work

Root help used to list about thirty peer groups in no particular order, and many jobs had two or three spellings. Now it is ordered by pillar: the work model first, then history and monitoring, then `ops`, config and the rest. Most of the change is moves, and `lf repo` is gone. (#1708)

- **Delivery and machine plumbing live under `lf ops`.** That covers `lf ops pr …`, `lf ops wt …`, `lf ops account …`, `lf ops machine …` (with `pool` and `ondemand` underneath it), `lf ops commit` and `lf ops sync`. `lf sync` stays a shortcut even though `task sync` also exists.
- **`lf repo`'s subcommands were redistributed:**
  - `release {run,check,notes,bump,tag,publish,status}` → `lf ops release …`
  - `ci` and `tokens` → `lf ops ci` and `lf ops tokens`
  - `identity` → `lf config repository`
  - `planning {key,status}` → `lf config planning {key,status}`
  - `connect` and `reteam` → `lf wave connect` and `lf wave reteam`
  - `new-chapter` → `lf wave new-chapter`
  - `refresh` → `lf wave status --sync`, which now also exchanges the Git planning ref. `refresh --all` and `release history` have no replacement.
- **Steering replaces comments.** `lf repo comment` and `lf wave comment` are removed. `lf session steer "…"` with no Session id steers the Wave chosen with the global `--wave`, or else the current repository. `lf task comment` is unchanged.
- **Repository scope comes from the global `--repo` selector.** `lf ops machine use` and `lf session steer` no longer have their own boolean `--repo` flag.
- **Every Task has a Workflow.** A Task with no Workflow checks its Wave default, then the repository `workflow:`, and then falls back to the builtin `code`. Desktop no longer shows "No workflow selected".
- **New builtin `auto` workflow.** It runs `pursue` and then `deliver` as one route, with no stage for a person. Use it to take a Task from request to merged PR with nobody stepping in.
- **The docs follow the same order.** The README explains a Task first (one conversation, a Workflow, its own PR, its history), then Graphs and Skills, then Waves, and ends with a "You want to…" table. `docs/` is rewritten as a reading path that starts at Get started.

## Scheduled work is ordinary cron

`lf wave cron` (add, list, publish, unpublish, sync, history, trigger, remove, disposition) is removed, together with its module, its test suite and the `cron` crate, about 8,000 lines in all. Scheduled telemetry and releases are now plain crontab lines that run `lf … graph release-run`. `release/CRON_HOST.md` and `scripts/bootstrap-cron-host.sh` show how to set them up. `lf self doctor` no longer runs a cron-continuity audit. (#1708)

## Agents finish what they started

Two separate problems could cut real work short. A headless Claude step used to settle at its first `result`, even when the agent had backgrounded a test suite and ended its turn to wait for it. Loopflow then shut Claude down, the command was killed, and the Graph moved on (the LOO-596 incident). Separately, `lf land`'s CI repair refused with "checkout is busy" whenever a Task Session or any other process was live in the checkout, and a sync held its lock through network fetches and conflict resolution. (#1688, #1714)

- **Headless Claude steps wait for background tasks.** A step stays alive while Claude reports running background tasks. It completes on the turn Claude starts after they finish, and only that final answer is recorded. Steers accepted in the meantime still settle. While background tasks run, the Session counts as busy. The headless instructions now tell agents to stop long-lived processes such as servers or watchers before they answer, because one that never exits keeps the step running. Codex and OpenCode were not changed. (#1688)
- **CI repair runs beside live work.** Landing no longer checks the checkout for running work before it starts `ci-fix`, so your Task Session stays open throughout the repair. Restoring a missing checkout no longer checks either. Abandon, `wt delete` and cleanup of completed Tasks still wait, because they remove the checkout. (#1714)
- **Sync holds its lock only while it merges or pushes.** `lf sync` fetches and plans before it takes the lock, and refuses if HEAD moved in between. It releases the lock while `sync-conflicts` runs and takes it back to verify and push. Verification checks only what gets pushed, so tracked edits from other writers no longer fail it. An HTTP push that stalls aborts after five minutes without progress. (#1714)
- **A blocked launch names the sync holding the lock:** `lf sync pid 4121 has held the sync lock for 14 min (feature onto origin/main)…`. (#1714)

## Launches go to the account with the most room

In shared mode, launches used to stay on the signed-in account until one of its usage windows reached 95%, and then follow declared route order. Running Claudes could sit on an almost exhausted account while another account had plenty left. (#1717)

- **Ranking:** eligible accounts sort by plan first (Codex Pro ahead of Plus). Within a plan, the account whose tightest unreset window has the lowest use goes first. Route order only breaks ties. The 95% threshold is gone. A window that has reset, or has no reset time, counts as full room.
- **Shared mode:** the signed-in account wins only ties. A launch switches every running Claude or Codex only when another account ranks strictly ahead. An explicit `--account` still wins, and the ranking applies within what it leaves open. Resumed provider sessions stay on the account that created them.
- **Switches are recorded first.** The switch is written to the store before the native login changes. If it can't be recorded, the launch fails and the current login stays in place, so `used_account` attribution stays correct.
- **`lf account route`** shows each account's tightest window (`weekly 80% used`) instead of a `demoted:` line.

## Operational notes

- **No store break.** This release adds no migration, and `lf` v0.14.17 can still open the store.
- **Update scripts and crontabs that call moved commands.** Short forms resolve, but scripted canonical paths such as `lf repo release run`, `lf repo ci` or `lf pr land` should move to the paths above. Anything that relied on `lf wave cron` schedules must be set up again as crontab entries that run `lf … graph release-run` (see `release/CRON_HOST.md`).
- **JSON rename:** in `lf account route --json`, the field `demotion` is now `tightest`. Update any consumers.
- **`LF_GIT_OPERATION_ID` is gone.** Conflict resolvers no longer need it, because the sync lock isn't held while they run. The macOS app and the environment docs no longer set it either. Lock records written by an older `lf` have no `locked_at`, so the refusal can't say how long those syncs have held the lock. The two versions still share the same lock file.
- **Known limits:** the push stall timeout covers HTTP remotes only, and a stalled SSH push can still hold the sync lock. If another writer runs `git add` while a conflict resolver works, that change lands in the merge commit. This is deliberate.
- **Upgrading from v0.14.17 fixes Task and Wave status.** v0.14.17 renamed the Graph field `human` to `interactive`, and Task status and Wave status then failed with `missing field interactive` on Graphs recorded earlier. v0.14.18 accepts the retained `human` name in compiled nodes and saved steps, including nested ones, and still writes `interactive`. Review stages and routes are preserved, and no records are rewritten. (#1722)

## Small changes

- Prompt goldens and the shared `StandInClaude` test fixture cover the background-task contract. (#1688)
- Short comments now state what each remaining lock protects: the worktree lease, journal append, credential lock and landing observer. (#1714)
- `docs/subscriptions.md` describes the plan-then-room ordering. (#1717)