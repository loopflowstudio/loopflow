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
| Load opens conversations, nothing selected | 23 providers in 8s (15:13:20–28), no click |
| Conversations ready when you reach them | Failed on main (14 of 23 checkouts untrusted); repaired below |
| No unrequested work | Stopped run's 3 Graph IDs unchanged, no Graph nodes, no branch commits; 22 commits are planning refs |
| Warm limit 16 | After walking all 21: 18 active (16 + 2 primaries), 5 detached |
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

Stale: `.lf/wave/product/MEMORY.md:968` still says eight warm; realign owns it.
Fixture left behind: `~/.lf/demos/loo672-12923a08ac/durable` and `~/.lf/demos/loo672-21be4cca92/durable`,
plus their entries in `~/.claude.json`. Runtime removed, all fixture processes stopped.
