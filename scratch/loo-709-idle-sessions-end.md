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
  visible grid redraw (live attach redraws only the visible grid). The file is
  keyed by Session because that is what a returning reader knows; one
  conversation has one last screen, overwritten by each owner's end. This
  branch delivers the writer and its test; the reader (`lf resume` painting
  it when stdout is a terminal) and the format upgrade are LOO-710's.
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

Replaced the executable-gone path’s store-only exit judgment in `lifetime::watch`
with a judgment that also preserves recent command output. Keep its quiet-provider,
active-turn and installed-symlink coverage on the same path. No schema change.

Already deleted: `lifetime::LONGEST_PAUSE`/`recheck`/`held_until` pacing (a held
judgement restarts the grace instead) and `limit()`'s "unreadable config keeps
the Session" branch. `Grace::due` is removed too: the policy needs only whether
the grace expired. Keep the output-only clock separate from the detached grace;
viewer changes must not extend the executable-gone quiet interval. Lifetime
fixtures name owners by their role so the preservation assertions stay readable.

## Concept review (2026-11-02, headless)

Reviewed the accepted experience against the implemented change. No new
concept for the person: the setting keeps its name, `lf resume` is unchanged,
`lf resume --all` narrows, closing and idling are one thing. The review found
one mismatch, repaired in the following implement pass:

- **Both exit causes preserve recent command output.** The executable-gone
  path now waits until the command has been quiet for one poll (2s), in
  addition to the shared store judgment. Its `last_output` clock changes only
  on command output; viewer changes cannot prolong a vanished build. The
  regression extends `sessions_end_when_their_lf_executable_goes` with an
  unviewed native turn that draws without a store activity record, survives
  removal, then falls quiet and ends even after a viewer reconnects. Existing
  quiet-provider, active-turn and installed-symlink cases remain. The new
  regression failed before the repair and passed after it.

Consequences recorded, no change needed:

- A clean shutdown delivers SIGTERM, which records the owner's exit, so after
  a clean reboot `lf resume --all` restores nothing; only a hard crash leaves
  `outcome IS NULL`. Reopening what Desktop showed before a clean quit is
  Desktop's pane restore through `lf resume` (LOO-672), unchanged here.
- An unviewed provider blocked on a permission prompt is waiting for a person
  and ends after the grace; `lf resume` shows the history and the person asks
  again. This follows from the 2m default, which stays Jack's to change.
- Screen files: one per Session, overwritten on each end, never removed.
  LOO-710 leaves pruning to this Task's retention; nothing in the store's
  Session retention deletes files today. Left as a known gap for whichever
  branch lands second, with the format.

## Remaining

- **Last-screen path and reader: agreed with LOO-710 (2026-11-02).** LOO-710's
  plan adopts this branch's directory and key, `<home>/screens/<session>`, in
  its own format (`.ghostsnp`), adds no DTO field, and owns the reader:
  `lf resume` paints the saved screen before launching the provider when its
  stdout is a terminal. Whichever lands second deletes the `.vt` writer
  (`transport::save_last_screen`/`last_screen_path`, `Output::last_screen`,
  `terminal::last_screen`) and rewrites the `transport_tests` decode to
  `.ghostsnp`. Jack Heart's steer asked the two Tasks to agree before either
  publishes; they have.
- PR body: before/after numbers below; point the 27 pre-rule owners at
  LOO-705's `lf monitor rollover`; say that painting the last screen on return
  arrives with LOO-710's reader, this PR writes it.

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

- `cargo test -p loopflow --lib transport::lifetime::` (1) and `cargo test -p loopflow --test transport_tests sessions_end_` (2) pass; `cargo fmt --check`, `git diff --check` and `lf context` pass; macOS linker warns about the large test binary’s unwind table; gate owns broader affected suites and Clippy.
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
