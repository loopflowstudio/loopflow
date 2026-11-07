# LOO-672 verify on main — November 7, 2026

Main `8d194ad32`; tree `9a6b8af3` is identical to this checkout's HEAD, and CI passed on it. No
production code changed between the November 7 native demo build (`8c6a706be`) and main,
so that demo's load, selection, draft, relaunch, reboot-like and Close results apply to main.

New native run: main's build, fresh disposable data, real Claude, 20 Tasks with no
Session, one with a prepared Session and a stopped run, one Wave, 23 conversations in all.
Setup: `uv run python scratch/prepare-verify.py`, then the printed root's `durable/launch.sh`;
`LF_DESKTOP=<bundle_id>` reaches its socket. `screencapture` and AX are denied in this
environment, so results come from `lf desktop capture/state`, `lf ps` and logs.

| Claim | Result |
|---|---|
| Load opens conversations, nothing selected | 23 providers in 8s (15:13:20–28), no click; past the accepted bound, see Warm bound |
| Conversations ready when you reach them | Failed on main (14 of 23 checkouts untrusted); repaired below |
| No unrequested work | Stopped run's 3 Graph IDs unchanged, no Graph nodes, no branch commits; 22 commits are planning refs |
| Warm limit 16 | After walking all 21: 18 active (16 + 2 primaries), 5 detached; at load, see Warm bound |
| Cold return | Warm 01: same provider PID, history shown, no new turn |
| Input | `lf desktop send` reaches the prompt after selection |
| Takeover | `lf session resume` from a second pty: Desktop's surface goes, action reads "open in another terminal", Take back offered, Desktop stays passive through re-selection |
| Take back click | Not driven: AX denied. The action runs the same `lf session resume`; Swift and transport tests pass in CI |
| Memory, 23 conversations | Desktop footprint 1.05–1.31 GB; Claude 222/302/453 MB min/avg/max, 6.96 GB total; lf 2.4 GB RSS (debug build); total RSS 9.98 GB |
| Eviction bounds memory | No: the 5 detached providers kept running, all PIDs unchanged. No budget is stated |

## The gap, repaired

`rust/loopflow/src/harness/claude_trust.rs`: `LOCK_WAIT` was 2s on `~/.claude.json.lock`,
so 14 of 23 launches gave up while the burst's booting Claudes held the lock. It is now 30s,
past `LOCK_STALE`, so a dead holder clears and only a live one can run the wait out; the
launch still falls back to Claude's dialog after that. `preparationLimit` 2 doesn't pace
this: an opening finishes before Claude boots. Claude 2.1.319 retries this lock with
backoff, then writes on a fresh disk read, so it does not drop lf's entry.

Rerun, November 7, same fixture, real Claude, `lf` built with the fix (Desktop binary from
06ab7bf4a; its Swift matches): `~/.lf/demos/loo672-21be4cca92`.

| Check | Before | After |
|---|---|---|
| Lock warnings in `logs/` | 14 | 0 |
| Fixture worktrees trusted in `~/.claude.json` | 9 of 23 | 23 of 23 |
| Task surfaces showing `trust this folder` | 13 of 16 read | 0 of 21 read, twice |
| Conversations with an assistant turn in their transcript | not counted | 23 of 23 within 2.5 minutes |

The stopped Task kept its 3 Graph processes and no worktree gained a non-planning commit.
The repository and Wave panes were not captured (`-p` needs a Task); their transcripts count above.

`cargo test -p loopflow --lib harness::claude_trust::`: 8 pass; the new burst test fails with a
2s wait. `cargo fmt`, `cargo clippy -p loopflow --all-targets -- -D warnings`: clean. Gate runs the affected suites.

Not checked here: whether a stuck dialog showed as waiting in Desktop; the session list
read `active`, and the rendered indicator can't be seen in this environment.

Launch path: both `accept_created_worktree` callers wait before spawning Claude, inside the
served child. The 5s `ATTACH_TIMEOUT` is the viewer's handshake with the owner, so nothing
shorter covers the 30s wait. A live holder past 30s still falls back to the dialog.

Known gap: two waiters behind a dead holder can both pass `_stale`, and the later `remove_dir`
can drop the earlier one's fresh lock. One trust entry can then be lost with no warning, and
that conversation shows the dialog. Not seen in a run; proper-lockfile has the same window.

Follow-ups: 634c19 (memory budget), d0071b (Wave links), 85c699 (first launch, no default agent).
Fixture left behind: `durable/` under `~/.lf/demos/loo672-12923a08ac`, `-21be4cca92`, `-8f72197490` and `-826bc2d6e2`,
plus their entries in `~/.claude.json`. Runtime removed, all fixture processes stopped.

## Warm bound, November 7

Accepted: sixteen recent unfinished Task conversations, panes on screen excepted; a load spends
"up to sixteen Task turns plus one per primary". Load opened all 21, because `visibleWork` was the
whole board, so off-page tiles counted as on screen.

Now `openWork` stays the board's members and `visibleWork` is the current page plus a selection a
turn left behind. `navigation.boardPages` holds every board's page (nil key: the unsaved view) and
`turnPage` re-presents. `prefetch` admits a tile on or off its page unless `releasedTasks` says the
bound or a window Close let it go, and counts tiles on screen against sixteen, so a refresh admits
nothing past the bound.
`releasedTasks` stays: `RepositoryWorkspaces` evicts across repositories, and an off-page tile keeps
its workspace, so a refresh would otherwise readmit what that bound just evicted.

Native, same fixture, real Claude, candidate with this diff: 18 providers at load, stable a minute
(16 + 2 primaries; was 23). Repository page, 3 slots, 7 pages: page 7 → 21, page 6 → 23, pages 5–1
stay 23. `open_work` stayed 22 throughout. Stopped Task kept its 3 Graph processes.

Departing tiles release their viewer; their providers keep running. Tiles in a hidden strip (the
default) count as on screen; the bound holds, at most one page early.

Memory budget not reopened: eviction stopping no provider matches the design ("protected work and
other hosts may keep the provider running"); memory cost was accepted November 4 and "no byte
budget" was disclosed before “ok lg2m ship it”. 634c19 holds the budget. LOO-672's text still lists
that acceptance, so finishing the Task means reading it against that Task.

`openWarmConversations` refreshes intent, location and completion for every open tile, not just
warm ones. Narrowing `visibleWork` had dropped off-page tiles from that loop, so `lf desktop focus`
on a tile past the bound never opened it in the test fixture, whose tiles have no location until
that loop sets one. A tile with a plan location would have opened with the default reveal intent (read, not run).
`focusedOffPageTileStarts` failed before this and passes. Selection and page turns already re-presented.

## Concept review, November 7 (second pass)

The accepted design holds and no product choice is open: load, selection and Close follow it,
and sixteen against "prefetch everything" is settled in `questions.md`.

One defect, repaired: a page turn let go of the freshest unvisited Tasks. Every prefetched Task
ties at `distantPast` in `RepositoryWorkspaces.reconcileWarmth`, so `recentTasks` order survived
and the head went first; `evict` marks a Task released, so refresh would not readmit it.
The design orders that set "selected Task, visible tiles, saved recent working Tasks, other working
Tasks by shared update time"; ties now give up its tail through an explicit rank.

21-Task board, `task-0` selected, turn to the last 5 tiles. Warm before: `task-0`…`task-15`.
Old order kept `task-0`, `task-6`…`task-15`; now `task-0`…`task-10`. `pageTurnEvictsStalestUnvisitedTasks`
fails without the rank. Return costs a reconnect, not a click, and the provider keeps running.

Read and left alone: `pages` returns `[[]]` for an empty board, so `presentBoard` can't index -1;
after a window Close only a visit readmits a Task, as on #1816, and the comment on `releasedTasks`
says so.

## Concept review, November 7 (third pass)

Nothing to change and no product choice open. Read against the code, not run: load, a visit past
sixteen, a page turn and back, Close, relaunch, a second repository, a finished Task. Sixteen is in
place, preparations stay at two, and no doc still says eight.

Two limits, left alone: a slot freed by a finished Task stays empty once every other Task has been
released, until a visit; if `lf desktop focus` leaves a Task selected on a Wave view, a page turn skips
`presentWaveTiles`, so a tile past the bound waits. A click can't get there: Wave tiles don't take focus.

## Headless demo, November 7

Run at the demo step with no person present; the decision was made standing in for Jack Heart, so
nothing here is his acceptance. Candidate built from `9253b35a4` (lf and Desktop), fresh fixture
`~/.lf/demos/loo672-826bc2d6e2`, real Claude: 21 Tasks, one Wave. `screencapture` and AX are denied, so
results come from `lf desktop state/capture/send`, `lf ps`, process lists and Claude transcripts.

| Claim | Observed |
|---|---|
| Load opens conversations, nothing selected | 10 owners at 6s, 18 at 16s, stable to 57s: 16 Tasks, repository, Wave. Warm 04–08 wait |
| Live before selection | Warm 01 captured, never selected, its first turn on screen; `lf session list` reads 18 active |
| Repository and Wave | Both reported status in their transcripts. Panes not captured, see below |
| Focus on an off-page tile past the bound (left open by the verify pass) | `lf desktop focus -p "Warm 06/session"`: Task entered, owners 18 → 19, surface live, `lf desktop send` text at its prompt |
| Page turn | Pages 7, 6, 1: owners 22, 23, 23; `open_work` stays 22 |
| No unrequested work | 19 first turns: 62 Bash calls, no Edit or Write, no `lf task run`, no commits on any branch; `lf wt prune` was a dry run, 23 worktrees stay. Stopped Task keeps 3 stopped Graph processes; `lf ps` shows no Graph node |
| Trust | 23 of 23 worktrees trusted, none showed the dialog in what was captured |

`lf session rename` ran twice (repository, Wave), which the launch guidance asks for. An untracked
13-byte `.gitignore` (`.lf/prompts/`) appears in each launched checkout at launch; it is not an agent's.

Read and cleared: `lf desktop state` lists Take back on a conversation Desktop itself shows,
because the record's holder is its owner process. `localTerminal` wins in `SessionsView`, so the
item stays `.live` and the terminal stays mounted; capture and send confirm it. Not seen rendered.

Not reached: the Wave and repository panes on screen. `capture -p session` needs a Task, and
`lf open` of the Wave or its Session lands on the repository (d0071b). Close, relaunch, reboot-like and
Take back were not rerun on this build; the November 7 runs above are the latest native results for them.

Decision: advance. Next: gate, then the running `lf ship`.

## Gate, November 7

On `9253b35a4`. No code changed; diff, docs and PR #1842 copy read against each other and left alone.

- `scripts/test.py --reuse-passing --rust-filter 'test(harness::claude_trust::)'`: architecture, rustfmt, nextest (8), clippy pass.
- Swift: 732 of 734 pass, which covers every test this branch changed. The two failures are `TaskFiles` `commitInLinkedWorktreeRereadsTheComparison` and `ignoredWritesFromReadsNeverScheduleAnotherRead`, again when run alone.
- Cause is this Mac: `fseventsd` pinned at 100% and a bare FSEvents stream, no repository code, got no event in 20s. No branch change reaches `TaskFiles`. Not passed: CI's `macos` job owns these two; it has not run on this head (skipped so far).
- `check_swift_multiplatform_boundaries.py`, skipped by the Swift failure, passes on its own.

## Remaining

- The trust-lock, warm-bound and eviction-order fixes publish as a new PR with #1816 as history.
- Proposed `.lf/wave/product/MEMORY.md` change (memory is at its token limit, and this pass leaves it unedited): in the November 7 note, after "no click or Graph", add "before the page bound (now sixteen Tasks, page included, plus primaries: 18 at load)".

Checks: `swift test --filter 'WorkspaceRetentionTests|WorkNavigationTests|WorkDestinationTests|WorkCacheTests|DashboardWorkspaceTests|DesktopPaneControlTests|DesktopActions'`: 197 pass.
