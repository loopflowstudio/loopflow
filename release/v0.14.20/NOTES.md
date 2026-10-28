# v0.14.20

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.20 lets one Graph do what Tasks used to need workflow stages and routes for: run its steps in place, wait in the Task conversation, or run everything headless. Live views now stream from the same commands you run once, so Desktop stops paying for a whole-store snapshot. Disk cleanup moves out of the binary into an hourly script that keeps unpublished work. Automatic sync and Codex launches now recover from two failures that used to stop work partway. This release raises the store compatibility floor: **lf < 0.14.20 can no longer open the store (raised by: task_graphs)**. Upgrade every CLI and macOS app that shares the store together.

## One Graph runs the Task, start to finish

Graphs used to stop at interactive steps, and Tasks continued through a separate model of workflow stages, routes and moves. That second model is gone. A Task now holds one captured Graph and its position in it, and the same Graph file decides where a person takes part. (#1733)

- **Interactive steps behave by launch.** `lf graph` runs `interactive:` steps in place. `lf task run` stops before one, leaving the Task waiting in its conversation, and the next `lf task run` continues after it. `-i` makes every step interactive and `-b` every step headless; otherwise each step keeps its own `agent:` choice.
- **Start anywhere.** `lf task run <ID> --from <step>` starts at a chosen step; choosing an interactive step leaves the Task waiting there. Desktop offers Start, Continue, Run again and Start from.
- **Runs restart cleanly.** A failed run starts again from where it began. Other Graphs run in the same checkout never move the Task's position.
- **Defaults resolve in one order.** A Task with no Graph takes up its Wave's Graph, then repository `graph:`, then builtin `code`, and keeps it once captured. Set a Wave's default with `lf wave graph set`; inspect it with `lf wave graph show`.
- **Shipping uses the same path.** `lf ship` runs the headless Task-run path. `lf ship` or any run into `end` approves Auto-Ship, which may land the PR; `lf ship --stop` withdraws approval.

## Follow live state with the command you already know

`lf monitor work` projected every Task, Session and process into one snapshot. On Jack's store (762 Tasks, 6,248 Sessions) a single idle reader used about 48 s of CPU per minute, and one outlived its app by more than three hours. Now any ordinary read streams: add `--watch --json`, and each watch reads only what its command reads. (#1719)

- **`--watch` on ordinary reads.** `lf task list`, `lf task status`, `lf session list`, `lf wave show`, `lf wave status`, `lf history`, `lf ps`, `lf planning status` and the `lf desktop` reads accept it. Each line is `{answers, unavailable, body}`, where `body` matches `--json`, sent at start and whenever it changes.
- **Watches stay cheap and exit.** A watch wakes on the store revisions it reads, rests five times the cost of each reading, and exits when stdin closes, its parent exits or a write fails. Writing `{"id": N, "args": {…}}` to stdin changes arguments; the next line answers `N`. A store that exists but cannot be opened reports `unavailable` rather than an empty answer.
- **Open Tasks by default.** Planning reads, `wave status` and `lf task list` read incomplete Tasks. `lf task list [query]` searches by title or identifier with `--wave`, `--project` and `--state`; closed Tasks page newest first with `lf task list --done --since 7d --after <next>`.
- **One Task row.** `lf task list`, `lf task status` and Wave reads print the same row: identifier, title, Wave, Project, one `state`, reason, workflow stage, PR and next command, with RFC 3339 times. `lf task status` adds Sessions, Graph processes and comments.
- **Desktop reads per part.** Desktop runs one watch per part over `lf desktop plan | plans | task | wave | done | sessions` instead of sharing one snapshot.
- **History and journal split.** `lf history` covers Session turns, commands and usage; work no Session did stays on the Task. The full planning mutation journal moves to `lf planning journal`, and `lf planning status` keeps receipts and held projections.

## Cleanup that frees space without losing work

Cleanup policy lived in three places: the lf binary, Session startup and a machine script. It now lives in one editable macOS script run hourly, and worktree pruning no longer deletes the only copy of committed work. (#1744, #1753)

- **An hourly script owns filesystem cleanup.** `scripts/disk-cleanup.sh install` replaces the launchd job with an hourly cron entry that runs the `disk-cleanup` skill from the main checkout. The skill reports reclaimed space, retained output, unfinished work and failures across repositories under `~/src`; it deletes nothing itself.
- **Idle build output is reclaimed.** Ignored, untracked build directories untouched for four hours are removed from main and linked checkouts that no process holds open. Returning to such a checkout may need a rebuild or dependency reinstall, even with unfinished source work.
- **Home material rotates after seven days.** Captures, logs, draft store copies and older backups rotate; the database, account homes and the newest backup per database are kept. Material is retained when open-file inspection fails.
- **Store retention moves to `lf monitor prune`.** It settles proven-dead Process rows, forgets unreferenced finished rows, compacts usage and reclaims database pages. Dry runs leave the store unchanged. The in-process rotation thread is deleted.
- **Pruning requires PR evidence.** `lf wt prune` removes a checkout only when a merged or closed PR matches its exact local head. Missing from origin, a week of inactivity and local merge equivalence no longer qualify, and missing PR evidence keeps the checkout. Dirty files, unfinished Tasks, persistent worktrees and live processes stay protected.
- **Removal rechecks before it acts.** Branch, head and cleanliness are rechecked under the removal lease; Git removal is non-forced, the local branch is deleted only if its head still matches, and remote branches are left alone. `lf wt delete --force` still disposes explicitly.

## Agents read the documents you select

Selected documents could appear in preview while the agent received only their paths. Turns now carry complete document bodies, and preview, provider input and capture match. (#1752)

- `--docs`, config `docs:` and skill frontmatter `docs:` share one assembly path for ordinary and native skills. References keep source labels and escape dollar signs so they don't invoke native skills.
- Explicit selections win over ambient duplicates. Resumed turns supply newly selected documents; earlier ones stay in native history.
- External absolute paths no longer crash against repository ignore rules. Canonical paths are deduplicated before the 100-file ceiling.
- Missing, unreadable or non-text selections report their path and reason; empty files stay as labeled references.
- Provider context windows and Pi/OpenCode terminal argument limits still apply, and Loopflow does not retry with shortened documents.

## Rust-only PRs land at Linux speed

Rust-only PRs waited 10–49 minutes for hosted macOS runners, once in the PR and again in the merge queue, and every land merged main into the branch, discarding the base CI had tested. (#1749)

- **macOS jobs run only when reached.** `swift-test`, `macos-process-test` and `loopflow-ui-test` run once per PR, each gated by the paths it can be affected by. A Rust-only PR runs none; CI changes, an unknown diff or a failed classifier run all three. Merge-group runs skip macOS.
- **main is tested after merge.** `macos-main.yml` tests each commit since the `last-green` tag. Green advances the tag; red bisects first-parent commits and opens an unarmed revert PR for the culprit.
- **`lf land` merges main only when needed.** That happens on a GitHub conflict, when GitHub requires an up-to-date head and no queue will supply one, or after a queue eviction for `merge_conflict`. Other repairs, including CI fixes, push on top of the tested base. Local merges, first publications and stacked children whose parent landed still merge main.

## Unattended work recovers instead of stopping

Two failures could halt work that nobody was watching: automatic `lf sync` aborting mid-branch on Apple Git 2.50.1, and Codex rejecting its active credentials while Loopflow held a working copy for the same login. Both now repair themselves without disturbing what they protect. (#1754, #1756)

- **`lf sync` leaves telemetry alone.** Sync checkpoints ordinary edits in a `Checkpoint edits before syncing onto <upstream>` commit, merges upstream and leaves `.lf/tmp/` exactly as it was, whether ignored, unignored or already staged. Explicitly staged ignored files stay in the checkpoint. Telemetry-only or absent edits make no checkpoint, so a repeat sync leaves `HEAD` unchanged.
- **Sync failures say where your work is.** A conflicting telemetry restore keeps the named stash and prints the exact `git stash apply --index <sha>` command. A held `index.lock` fails before touching `HEAD`, the index or the working tree; an index-refresh failure after the commit says the checkpoint was saved.
- **Codex credentials repair in place.** Status, readiness and reset redemption use verified access. When Codex definitely rejects its credentials and an accepted copy exists for the same login, Loopflow repairs credentials in the conversation's existing home, so Session identity, native thread and history are preserved. Identity mismatches or unavailable verification never trigger the fallback.
- **Running clients aren't clobbered.** An atomic exchange retains displaced credentials, including later writes from an already-running client, and save-back requires verified access. Account routing is preserved, cached status makes no provider requests, and continuing a completed Session keeps its completion record and prior captures.

## Operational notes

- **Upgrade the store's readers together.** The `task_graphs` migration raises the minimum reader to 0.14.20. Update the CLI, Desktop helper and cron installations that open the store. It replaces `task_workflows` with `task_graphs`, renames the Wave default column and drops workflow move history; Graph process records and Task events remain. (#1733)
- **Update scripts for removed commands.** `lf wave workflow` (and `source`, `list`, `show`, `set`) becomes `lf wave graph` / `show` / `set`. `lf task workflow` (`move`, `show`, `restart`) and `lf graph route` are removed; use `lf task run`, `--from` and `lf task status`. `lf monitor work` is replaced by per-read `--watch`; `lf monitor active` by `lf session list --watch` with `lf ps --watch`. `lf disk` and `lf disk clean` are removed in favor of `scripts/disk-cleanup.sh` and `lf monitor prune`. (#1719, #1733, #1744)
- **Repository `workflow:` still works for now.** It remains an alias for `graph:` until a release supporting `graph:` is installed; LOO-637 tracks its removal. (#1733)
- **Install the cleanup schedule.** Run `scripts/disk-cleanup.sh install` from the main checkout, then check the cron entry and `~/Library/Logs/loopflow-disk-cleanup-skill.log`. Real laptop cleanup, reclaimed space and cron installation were not verified before merge, nor was installed acceptance of unpublished-work preservation. Checkouts already lost to the old pruning rules are not recovered. Checkouts without matching PR evidence now need explicit disposal. (#1744, #1753)
- **Watch for macOS regressions on main.** A macOS-only break from a classifier-skipped PR now surfaces after merge. The bisect-and-revert path opens its revert PR unarmed, so someone has to merge it. (#1749)
- **Installed document delivery is unverified.** Branch receipts show complete Codex capture. Infrastructure owns repeating relative, absolute, external and resumed-turn selections through the installed build. (#1752)
- **Installed sync and Codex recovery are unverified.** After installing, Infrastructure checks that LOO-661 completes automatic sync on Apple Git 2.50.1, and that Mini's shared-home `lf account codex --json`, a configured headless Codex turn and `lf -b session connect <session-id>` continue on the same native thread. Telemetry committed before this release stays in history. (#1754, #1756)

## Small changes

- `docs/lf-reference.md` commits only its authored preamble; `website/dev.py sync-docs` appends the command tree from `lf help --all --json`. (#1749)
- `ops/task.rs`, the `human_session` tests and `docs/architecture-reference.md` are split into smaller files so parallel PRs conflict less. (#1749)
- Task details, Auto-Ship facts and Graph execution are read in batches instead of one query per Task. (#1719)