# LOO-709: An idle conversation costs nothing

Jack Heart's goal (2026-11-02): close or abandon any conversation and have it
cost nothing. Today every interactive owner stays resident until
`session.end_detached_after` (24h), and on 2026-11-02 that left 39 idle Claude
processes holding 8.7 GB.

## Why the 24-hour rule did not end them (2026-11-02 evidence)

56 `lf session serve` owners were alive on Jack's machine; 27 were older than a
day. Causes 1–3 were confirmed against the live store; 4–5 come from the
LOO-672 audit (2026-11-02) and `lf.log`:

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

4. **One store error killed the policy.** `lifetime::watch` propagated store
   errors with `?`; the thread died ("Session owner policy stopped") and the
   owner lived on with no policy. `lf.log`: 43 `database is locked` deaths on
   10-27, 34 `incompatible Loopflow database` deaths on 10-30 after 0.15.1's
   migrations, and no "ending a Session nobody has viewed" line ever.
5. **A stale status report reads as a turn forever.** `TURN_RUNNING` counts a
   terminal `program_status` with no idle/blocked/done record as running, so
   a provider that stopped reporting made its owner immortal. The owner now
   judges by `TURN_LIVE`; `TURN_RUNNING` remains only for `lf resume`'s
   "a turn was cut" report (`session_turn_was_running`).

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
  not. A store error is logged once per distinct failure and judged again
  next poll; the policy thread never exits on one. A turn counts only from a
  live AgentProcess on its current attachment, and a terminal status report
  counts only while the owner is viewed (`TURN_LIVE`).
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

Deleted: `lifetime::LONGEST_PAUSE`/`recheck`/`held_until` pacing (a held
judgement restarts the grace instead) and `limit()`'s "unreadable config keeps
the Session" branch.

## Remaining (realigned 2026-11-02: code and plan agree)

- **Decision (Jack Heart / LOO-710): one last-screen path and reader.** This
  branch writes `$LF_HOME/screens/<session>.vt` with no DTO field; LOO-710
  writes `<home>/snapshots/<owner>.ghostsnp` and adds `last_screen` to the
  Session DTO. Unresolved; must agree before either publishes. See
  `scratch/questions.md`.
- PR body: before/after numbers below, and point the 27 pre-rule owners at
  LOO-705's `lf monitor rollover`.

## Done when

- `cargo test -p loopflow --test transport_tests` covers: finished Graph and
  finished step membership no longer hold an owner; a stale status report
  does not hold one; a turn and fresh terminal output do; a headless Graph
  step and a plain `lf -b skill … --message` Run outlive a 2s grace and settle
  `succeeded`; viewed-only `resume --all` after a crash, including rows
  retention already closed; screen file written at idle end and at command exit.
- Live check on this machine: open 20 terminal conversations, detach, wait the
  grace; `footprint`/`ps` show none; `lf resume` reopens each at the bottom.

## Check results

- Tree `a3160bd07b` (no code change since): `cargo test -p loopflow --lib --
  transport::` (15) and `--test transport_tests` (20, incl. `headless`,
  `resume_refuses`, `detached_sessions_end_after_their_limit`,
  `resume_all_restores`) pass; `cargo fmt` and
  `cargo clippy -p loopflow --all-targets -- -D warnings` clean.
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
