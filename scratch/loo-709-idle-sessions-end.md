# LOO-709: An idle conversation costs nothing

Jack Heart's goal (2026-11-02): close or abandon any conversation and have it
cost nothing. Today every interactive owner stays resident until
`session.end_detached_after` (24h), and on 2026-11-02 that left 39 idle Claude
processes holding 8.7 GB.

## Why the 24-hour rule did not end them (2026-11-02 evidence)

56 `lf session serve` owners were alive on Jack's machine; 27 were older than a
day. Three distinct causes, each confirmed against the live store:

1. **Finished Graphs hold their owner forever.** `owner_waits_for_person`
   walks `graph_processes` ancestry and `graph_process_steps` membership with no
   liveness test, so once any Graph has run under an owner (a Task session that
   ran `lf task run`, `lf -b skill …`) the owner never reads as waiting for a
   person. Five of seven sampled owners from the 2026-10-30 cluster were
   ancestors of only finished Graphs (all `completed_at` set).
2. **The limit counts awake time.** `lifetime::watch` schedules `due` with
   `Instant`, which on macOS stops during sleep. Since boot this machine has
   been awake 170 of 266 hours; a weekend stretches "24h" to days. The two
   sampled owners that read as free were still inside their awake-time limit.
3. **Owners from before the rule.** The rule landed 2026-10-29 (#1765, v0.15.1).
   Owners up 4–6 days run copied builds under `~/.lf/bin/lf-<hash>` that have
   no lifetime policy at all, and nothing but a signal ends them (e.g. the
   LOO-618 owner, `lf session connect … --try`, up 4.9 days).

Also observed: `limit()` returns `None` when the repo config fails to load, so
an unreadable config silently disables the rule.

## Design

Rust only. Desktop (LOO-672), per-process overhead (LOO-708) and snapshot
fidelity (sibling Task) stay out.

- **Idle ends quickly.** `session.end_detached_after` keeps its name; default
  drops from `24h` to `2m`. Jack named 2m and 30m as candidates; 2m is chosen
  because the goal is that an unviewed conversation costs nothing, and
  `lf resume` pays only a provider restart. The owner judges by wall clock
  (`SystemTime`), so a laptop waking from sleep ends its idle sessions within
  the next poll. Unreadable configuration uses the default instead of
  disabling the rule. A turn in progress, a *running* Graph (ancestry or step
  membership) or a headless Run still holds the owner; a finished Graph does
  not.
- **The last screen survives.** Before a terminal owner exits (idle end,
  executable gone, or its command exiting) it writes
  `$LF_HOME/screens/<session>.vt`: the vt100 scrollback rows followed by the
  visible grid redraw, the same bytes a viewer receives on attach. Desktop and
  `lf resume` readers paint it before the provider is back. Format is the
  sibling Task's to upgrade.
- **Return is cheap and starts nothing.** Unchanged: `lf resume <id|task>`
  reopens from provider history (`claude --resume <id>`), lands at the bottom,
  replays nothing.
- **Restart restores only what was being looked at.** `lf resume --all`
  resumes Sessions whose owner never recorded its own exit (`outcome IS NULL`,
  so retention closing a dead pre-boot row does not hide it), is gone from the
  OS, and had a viewer when it died (`processes.viewed`). Busy-but-unviewed
  owners are not resumed; their cut-off turn is not replayed anyway.
- **Closing is the same as idling.** No new command.

## Delete — do not maintain

- `lifetime::LONGEST_PAUSE`/`recheck`/`held_until` pacing: gone. A judgement
  that finds the owner held is activity and restarts the grace, so a busy
  owner is asked once per grace, not on its own schedule.
- `limit()`'s "unreadable config keeps the Session" branch: gone.

## Done when

- `cargo test -p loopflow --test transport_tests` covers: finished Graph and
  finished step membership no longer hold an owner; viewed-only `resume --all`
  after a crash, including rows retention already closed; screen file written
  at idle end and at command exit.
- Live check on this machine: open 20 terminal conversations, detach, wait the
  grace; `footprint`/`ps` show none; `lf resume` reopens each at the bottom.

## Check results

- `cargo test -p loopflow --test transport_tests` (20 tests) and
  `cargo test -p loopflow --lib -- transport:: detached_sessions_end rollover restore human_session::tests`
  pass; `cargo fmt`, `cargo clippy -p loopflow --all-targets -- -D warnings` clean.
- Compress: `cargo test -p loopflow --lib -- transport::` (15), `--test transport_tests
  detached_sessions_end_after_their_limit resume_all_restores` (2), clippy clean.
- Live, 2026-11-02, branch debug build in a throwaway `LF_HOME`, real Claude
  Code 2.1.316, this worktree as checkout: 20 conversations opened, each
  viewed once and detached at 13:53:07. Before: 60 processes (20 owners, 20
  launchers, 20 claude), 9.9 GB RSS. All 20 owners ended by 13:55:23 (grace
  2m), outcome `interrupted`/130, 0 of the 60 processes left, 20 screen
  files written, 20 "ending a Session nobody has viewed" log lines, no store
  errors. Three `lf session resume <id>` each relaunched
  `claude --resume <thread>` with no input replayed; the saved screen shows
  the Claude welcome and the pending prompt.
- The 27 pre-rule owners on Jack's machine are untouched; LOO-705's
  `lf monitor rollover` is the path for them.
