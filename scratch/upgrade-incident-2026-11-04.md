# Upgrade incident, 2026-11-04 20:02 PST

`lf install` (5519340b4) applied `settings_authority` and `task_session_promotion` while many older lf processes were live. Checked at about 20:25 PST.

## Works now (replayed from a pty with no inherited LF_*)

- `lf -a claude` and `lf -a codex`: launch, quit, exit 0, resume hint, no `no such column`.
- `lf resume <id>` plus quit on the new build, exit 0.
- `lf doctor`: store compatible, migrations at `task_session_promotion`, install selection and fallback ok. Public `lf` and the app helper are both 5519340b4.
- Account routes: `manabot-eng` in `/Users/jack/src/loopflow/lf.config.local.yaml` resolves as an email prefix. The cutover wrote only three files, so no other repository holds exported ids.

## Still broken

- Bare `lf`, interactive and `-b`: "No agent selected". Reproduced.
- Any Task or Graph step without an agent choice, unless its skill names one in `.lf/config.yaml` (explore, kickoff, prompt, realign, xor-route).
- Sessions on old builds: 134 processes in `lf monitor rollover --dry-run` (read-only): 29 movable, 22 held by native input, 3 held by a running Graph, 3 Graph runners, 77 that keep their lf until exit. Re-run it for current counts.
- Three Graph runners will fail at their next step: pid 68435 (manabot fc6ffb-separate-learning-seats), 18833 (manabot mtg-241), 95704 (manabot 5da5f4-design-effective-state).
- No LOO-714 or LOO-722 Session is in the rollover move list.

## "No agent selected"

Cause: #1805 deliberately removed implicit Codex-first selection ("Removes implicit Codex-first selection"). `default_agent()` used to return the first installed of codex, claude, opencode, pi; it now needs exactly one installed, and this Mac has four. The LOO-722 launches inspected carried no `-a`, and its recent agent Processes are all codex, which fits that fallback.

LOO-722: after the install, `implement` failed on the old build ("this store needs an lf that knows task_session_promotion"). The relaunched `verify` then failed "No agent selected" on all three attempts and the run stopped there. It is idle at `verify`. Its log records `verify` decided `iterate` for Bun cache maintenance, so that work is still open.

No saved default was lost in the cutover. `scope_settings` and `scope_defaults` had 0 rows in all four snapshots (14:49 to 19:56), the 19:56 `config.yaml` has no agent, `settings-cutover.json` exports none, and the Process ledger holds no `lf set default_agent`.

Two things sit beside that:

- Memory recorded `agent: claude` and `supported_harnesses: [claude]` in `~/.lf/config.yaml` on 2026-10-19. The 12:08 backup today has neither, and the named `.bak-2026-10-19` is gone. When it left is unresolved, but it predates the cutover.
- `settings_authority` drops `tasks.agent` without exporting it, and no database backup exists. Task-level `-a` choices are gone. Reconstructed from Process history, at least 11 unfinished Tasks had one: KAT-32, LOO-312, 515, 596, 599, 617, 672, 673, 732, 733 on claude; LOO-714 on codex. This is the last `-a` per Task, a reconstruction rather than the stored value. Today those Tasks stop with the error. Once `default_agent` is set they run on it, so LOO-714 would move off codex if the default is claude.

## Account selector repair

`match_account` is correct: its test pins that an account with an email is not selected by id (#1355). The defect was the cutover exporting ids into `account:`. `rust/loopflow/src/config/migrate.rs` now exports the login email, keeping the id only for an account without one.

Verification: `config::migrate::` 5/5; the new test fails without the fix (`left: "loopflow"`, `right: "jack@loopflow.studio"`); `cargo fmt` and `cargo clippy -p loopflow --all-targets -- -D warnings` pass. Uncommitted and not installed. This Mac needs nothing (config already hand-repaired); it protects Machines that have not cut over.

## Why it can recur

`settings_authority` is a single-step destructive drop (`machines.route`, tables, `tasks.agent`), against Jack Heart's 2026-10-27 direction to stage these: expand, about 30 days, delete data, drop. The floor check runs only at open, so live processes keep old SQL. The migration is on main and immutable, so this needs a forward process change. `wave_removal` also drops a table (`DROP TABLE waves`), which fits it as the first occurrence but proves nothing; still unconfirmed.

## Decisions for Jack Heart

1. `default_agent`: `lf set --global default_agent claude` or `codex`. Old effective behaviour was codex; memory says Tasks run on Claude only.
2. Old Sessions: whether to run `lf monitor rollover` (29 owners, ends them; it requires that nobody is viewing the Session, but whether it preserves an unsent draft on these old builds was not checked). The 22 native-input Sessions need the manual path: Ctrl-C the dead client, wait for old lf to exit, `lf resume`.
3. LOO-722: relaunch with an agent. Not launched here.
4. The 11 Tasks: restore choices or let `-a` decide per run. There is no per-Task setting any more.
5. Whether to land the `migrate.rs` fix.

# Why it happened (5 Whys, 2026-11-04 about 20:50 PST)

Everything above stays as the unbreak step recorded it. Corrections are listed first. Nothing below has been applied: no store change, install, commit or release. All sections after "Evidence" are proposals for Jack Heart to accept or reject.

## Corrections to the notes above

- **"No database backup exists" is too strong.** `~/.lf/backups/manabot-wave-cleanup-20261028T183047Z/loopflow.db` is a hand-made 10-28 copy that still has `tasks.agent`: 154 of 858 Tasks set (119 claude, 35 codex).
- **The list of 11 unfinished Tasks is wrong.** LOO-596, 599 and 617 are canceled and abandoned. The 10-28 copy adds LOO-558 (claude) and MTG-175 (codex), both started and missing from that list. It confirms KAT-32 and LOO-312 on claude.
- **`tasks.agent` was dropped by design, not by oversight.** `.lf/wave/infrastructure/MEMORY.md:862` says LOO-714 "supersedes saved Task-agent policy with per-invocation resolution". What was not decided in the open is that the values would disappear rather than be exported.
- **The "unresolved" `agent:` removal is resolved.** In the LOO-714 transcript at 12:30 today Jack said "i deleted tat since my claude has room again", then on seeing `default_agent = codex · Built-in`, "that feels wrong" and "agreed, lets update that". So #1805's removal of Codex-first selection, and the missing global agent, were both Jack's choices. The incident's "No agent selected" is the expected result of them, reached by Tasks that had no other choice.
- **`wave_removal` did not lose a table.** Its `DROP TABLE waves` is a rebuild to change a CHECK, with the columns kept. Old SQL would still have worked. Old lf failed anyway, because the floor refused it. This matters for prevention below.
- **"The floor check runs only at open" is half the story.** Long-lived processes reopen constantly: `lf.log` at 20:02:56 shows Session telemetry writes refused with "this store needs an lf that knows task_session_promotion". Old processes meet both the floor refusal and missing columns.

## What failed, who it affected, recovery

Jack's live work on this Mac. The install log listed 177 processes still on earlier builds. 23 idle Codex clients lost their relays at once and 22 timed out at 20:04:55. Quit paths failed with `no such column: route` on 10-hour-old processes (5 between 20:04 and 20:13). Graph steps failed through 20:13. Recovery is unchanged from above: LOO-714 and the 27 Codex Sessions are still on old builds.

## Causal chain

**1. Why did 23 connections reset at once?** The old relay polls the store and treats a store error as fatal. `codex_connection.rs` at ac8e088f5 runs `updates.pending` on a 1-second `update_clock` while a Session is idle, and any error but `InvalidAuthority` is `return Err`. The first reset is at 20:02:55.002, 92 resets across 23 clients in that 5-second bucket, against a commit at 20:02:54. Reconnects then hit `serve_with_input`, which reads the store before `accept_async`, so each retry resets during handshake until the 2-minute timeout. This is observed, not inferred: `session_events` holds 317 `codex_relay` failures across 20 Sessions from 20:02:55 to 20:07:50, all `no such column: route` in `SELECT tw.id FROM tasks tw JOIN agent_sessions a …` (`task_work.rs:560` at ac8e088f5, the Task lookup that `updates.pending` makes).

**2. Why could a store change reach running processes at all?** One SQLite file is shared by every build, and a migration takes effect the moment it commits. `settings_authority` also removed `machines.route`, and `store/sqlite.rs:266` at ac8e088f5 embeds `route='local'` in shared queries.

**3. Why was a breaking migration allowed while 177 processes were on old builds?** Nothing in the path can see them.
- `decide()` in `install.rs` takes compatibility and executable evidence. It never reads `raised_reader_floor` or running processes, so a floor raise is the same `PromoteAndMigrate` as an additive change.
- `promotion_lock` has only `acquire_exclusive`; no live process holds anything against it.
- `report_after_promote` runs the process inventory only after the store has advanced. The data for a pre-check exists; it is just asked too late.

**4. Why did nobody see it coming?** Because the warning is unreachable in a normal install.
- `render_human` runs only when `preview_only` is set. A comment from #1649 explains: "Newcomers see only the outcome". So the MIGRATIONS.md promise "Install previews name the breaking migration" holds for `lf install preflight`, not for `lf install`, and `lf install --preview` now means the release stream.
- What LOO-714's session ran, with `LF_*` stripped and stdin closed: `lf install`. `/tmp/loo714-install-retry.log` has no floor line.
- The one signal came after the commit, on line 3 of 57: "still running the previous lf: … can't open the store". The session read `tail -40`. That string occurs 0 times in its transcript, so it never saw it.
- That line also reassures wrongly: "each ends as its Session goes idle and `lf resume` reopens it". Today resume alone only reattached to a dead client.
- The session's own `ps` at 19:58 did show `lf` processes 10 hours old. Nothing told it those mattered.
- Its verification was all fresh `lf config show` runs. #1805's rollout section lists preference survival and names no running-process check, so "All final checks passed" was true of what it tested.

So what could have stopped it: nothing in code. A person running `lf install preflight` by hand would have seen one line. Jack's approval ("looks good! lets ship it and migrate old configs", 14:16) was about preferences; no message to Jack named Session impact.

**5. Why was the migration written as a one-step drop?** The repository told the author to. MIGRATIONS.md still reads "Rename, drop and reshape directly; the floor rises at that migration" (from #1608, 10-20). Jack's 10-27 direction to stage these exists only in a Claude memory file and a `scratch/faster-ci.md` that is gone. It appears nowhere in `MIGRATIONS.md`, `AGENTS.md`, `docs/architecture/data.md`, `check_migrations.py` or git history. LOO-714 ran on Codex, which never loads that memory. `build.rs` classifies a drop as breaking and raises the floor silently; `-- breaks:` is required for nothing.

**6. Why has this kept recurring after being fixed?** Each fix addressed recovery or wording after the skew, never whether the skew happens. LOO-548 (10-19, unstarted), LOO-502, LOO-693 (#1797: surfaces skew as an error), LOO-705 (#1799: moves idle owners, explicitly "the one Task for version skew"), LOO-724, LOO-723. LOO-705's sketch of a thin stable owner, the option that would have changed this outcome, was not what shipped. Its Task text says rollover "moves none" of the owners that cannot read the store, and that was still true tonight for LOO-714 and LOO-722.

## The missing backup

It was made, then deleted on purpose, seconds later. Migration runs through `apply_sqlite_with_backup`, which clones the store. `settle_switch` (`install.rs:2506`) then calls `prune_migration_backups(store, 0)`, with the comment "The settled release opens the store, so nothing restores its pre-migration clone." That came from #1666 (LOO-570, 10-24), whose goal was shrinking a 9.8 GB store and making install take seconds. The inference is from code and the settled receipt; no file remains to prove it existed.

The premise is the flaw: "opens" is not "kept everything". The clone only protects against a failed install, and it is deleted just when a destructive migration's data loss becomes knowable. MIGRATIONS.md was never updated, and "the two newest backups stay" is wrong against both `kept` values (1 after commit, 0 after settle). At 540 MB the original reason for deleting is also gone.

## First occurrence

The best-supported first occurrence is **11-04 09:16**, `wave_removal`. "Second" is true of mass Session loss and undercounts the class.

The Codex logs could not answer this: `~/.codex/logs_2.sqlite` is rolling and starts at 19:04 today, which is why it held only this event, and per-account logs cover only 10-27 to 10-28. The better source is `session_events`, which keeps each relay's failure. Its `codex_relay` records begin 10-20 20:53, so earlier relay failures are unknowable.

| When | Floor | Relay store failures (records / Sessions) | Other evidence |
|---|---|---|---|
| 10-19 | 0.14.8 era | not recorded yet | LOO-548: Flow died mid-run |
| 10-20 to 10-21 | 0.14.11, .12 | 10 / up to 2 per hour | Graph failures; `no such column` at quit on a 15.6h process |
| 10-23 | 0.14.15 | 1 / 1 | `no such table` on a 73.6h process at 10-25 |
| 10-26 to 10-28 | 0.14.16, .17, .19 | 40 / at most 5 in an hour | About 30 Graph and step failures |
| 10-30 05:34 | **unchanged** (`ship_stop`, additive) | none | LOO-705: "every one of them said delete loopflow.db"; 5 Graphs died. Contradicts "additive installs are safe"; unexplained |
| **11-04 09:16** | `wave_removal` | **9 / 8** | CLI-only `lf self install --dev` left Desktop on the 10-30 helper. Desktop failed at launch, 123 Processes on that helper, Jack reported reopened Sessions "stopped accepting input or crashed" |
| **11-04 20:02** | `settings_authority` | **317 / 20** | This incident |

11-04 09:16 is the only earlier event above 5 Sessions at once, and it matches Jack's description. Whether Jack also saw "Reconnecting to server" then can't be shown. Its direct cause differs: the app was not promoted with the CLI (fixed by #1815), and it failed by floor refusal, not missing columns. Each earlier row is the same class at a scale small enough to read as a one-off, which is part of why it kept being fixed one symptom at a time.

## Open questions

- Why the 10-30 additive install broke 0.15.0 readers, given the ledger floor did not rise.
- Why quit leaves the terminal unresponsive to Ctrl-C and Ctrl-D. The failing write is known; the hang was not traced.
- Why the top-level `lf install` at 19:57 has no Process row when `lf self install --dev` at 09:15 does. The riskiest command of the night is missing from the ledger.
- Whether the retained fallbacks matter: `lf-1bde30f` and `lf-676ce77` fail `install preflight` at every check, so the published fallbacks can't open the store and rollback is not a real option after a floor raise.

## Proposed prevention

Ordered by how directly each removes a cause. Jack rejected locks and gates three times in one day in favour of restructuring, so a hard install refusal is not proposed; 3 only holds back the unattended path.

1. **Decide about the 09:00 refresh, before 09:00 tomorrow.** `~/.lf/config.yaml` has held `stream: dev` since the 10:25 attempt today, and `com.loopflow.refresh` runs `lf install` daily at 09:00 and at login. That would be the first unattended install of newest green main: it replaces Desktop with nobody present, and any floor-raising migration merged tonight applies the same way. `main` has no new migration past 5519340b4, and the four open PRs with migrations show no DROP, RENAME or `-- breaks:`, so tomorrow's run should not raise the floor. Whether `gh` works under launchd was not checked. Choices: `stream: preview`, or unload the agent until 3 lands.
2. **A passive poll must not end a conversation.** In the relay, make store errors in the `update_clock` arm non-fatal, and stop reading the store before accepting a reconnect. Small, addresses cause 1, and protects every future skew. Test: a relay whose store starts failing keeps its client connected. Helps only builds from then on.
3. **Stop unattended and silent floor raises.** Pass the floor and the running-process inventory into the install decision; print the preview on every `lf install`; let the scheduled path proceed only when the verdict raises no floor. Test at `decide()`. Reword `report_after_promote` to what is true, and print it last so `tail` sees it.
4. **Put the staging rule where every agent reads it.** Replace the "drop directly" line in MIGRATIONS.md with Jack's rule, and have `check_migrations.py` reject a non-additive slug without a named expand step. Test: a fixture `DROP COLUMN` fails the check. The tension to resolve first: `wave_removal` shows SQLite CHECK changes rebuild tables and raise the floor even though old SQL still works. Staging drops alone would have stopped 20:02 and not 09:16's refusal, so the floor needs to mean "old SQL breaks", not "the file changed shape".
5. **Keep the clone past settlement when the floor rises**, for a set period, and correct MIGRATIONS.md. Test: settling a floor-raising switch leaves one clone. Cost is about store size as blocks diverge.
6. **Add a running-process line to rollout checks.** Any install in a rollout records `lf monitor rollover --dry-run` before and after, with what will not survive said to Jack before proceeding.

Longer term, LOO-705's thin stable owner remains the only option that removes the skew rather than managing it. Whether that replaces 2 to 4 or follows them is Jack's call.

Not proposed: restoring `tasks.agent`. The design retired it, the 10-28 copy is the only source, and it is a week stale. Decision 4 above still stands, now with four live Tasks rather than eleven: KAT-32, LOO-312, LOO-558, MTG-175, plus the process-history entries for LOO-515, 672, 673, 714, 732, 733.

# Execution plan (launch-plan, 2026-11-04)

A proposal for Jack Heart. No Task, setting, launch agent, commit or install changed. Jack has not approved creating Tasks.

## Corrections to "Proposed prevention"

- **`stream: preview` is not a clean 09:00 option.** v0.15.1 is the newest release and the installed build is ahead of it, so `lf install` on preview would try to install an older build. From `decide()` it should be rejected as unable to operate the store, so it protects only by failing. Read from code, not run.
- **Unloading alone does not cover login.** The plist has `RunAtLoad` and stays in `~/Library/LaunchAgents`. It needs `launchctl disable` as well as `bootout`.
- **"Four open PRs" missed a fifth.** #1808, an unarmed auto-revert of #1803, removes `wave_removal.sql`, which this store has applied. #1819, an unarmed auto-revert of #1815, restores the CLI-only install behind 09:16. Neither has auto-merge.
- **Proposal 3's scheduled hold conflicts with recorded constraints.** LOO-693: "needs no guard and no process rollover" (an earlier draft of this line misquoted it as "A refusal to migrate would be a guard", which appears nowhere in that Task). LOO-705: "no guards or refusals". It is split out for an explicit decision.
- **The staging rule does not wait on the floor question.** Only the classification of rebuilds does.

## Tasks (infrastructure Wave, one PR each)

| # | Outcome | Depends on |
|---|---|---|
| A | Idle conversations survive store errors (relay) | nothing |
| B | Every `lf install` says what it will break, and is recorded | nothing |
| C | Scheduled installs wait for a person when the floor rises | Jack's yes; B |
| D | Agents are told to stage destructive migrations; CI enforces it | nothing |
| E | The floor means "old SQL breaks" | Jack's floor answer; the 10-30 explanation |
| F | A floor-raising install keeps its restore point | nothing; rebase after B |
| G | Cutover exports login emails (`migrate.rs`) | Jack's yes to land |

Existing Tasks: LOO-705 and LOO-693 are done, so not reused. LOO-548 stays as the home for the thin stable owner, retitled when Jack chooses it. LOO-723 owns the #1819 decision before it completes.

Overlap: #1819, #1813, #1812 and #1656 change `install.rs`. No open PR found changing `codex_connection.rs`, `MIGRATIONS.md` or `check_migrations.py`.

Before any `lf pr submit` from this worktree: `lf scratch clear` removes this added file. Carry the evidence into Task briefs or a durable document first.
