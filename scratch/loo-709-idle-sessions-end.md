# LOO-709: An idle conversation costs nothing

Jack Heart's 2026-11-02 goal: closing or abandoning a conversation leaves no
resident agent. His continuation and review steers retain idle ending, require
silent native turn preservation, and keep the LOO-710 snapshot agreement.
Rust only; Desktop (LOO-672), process overhead (LOO-708), and snapshot fidelity
(LOO-710) stay with their owners. Earlier design and concept review:
`9bc58f47ba39ee1efaf0cc6677a37095d93d187f:scratch/loo-709-idle-sessions-end.md`.

**Status, implemented 2026-11-02:** silent-turn protection from `bafb3efdcd`
is preserved. Native Stop now retains background tasks and scheduled wakeups,
including while a viewed terminal reports idle; missing snapshots cannot settle
known work. Real Claude callback evidence and focused lifetime regressions are
recorded below. The 2m default and viewed-only crash selection remain the
assumptions in `questions.md`. Full return acceptance, gate and installation
remain unproved.

## Required behavior

- `session.end_detached_after` defaults to `2m`, supports `off`, and measures
  wall time with `SystemTime`. Unreadable config uses the default. Viewer
  changes, command output and held work restart the detached grace.
- A current-attachment turn, running Graph (ancestry or captured-step
  membership), or headless Run holds its owner. Finished Graphs and stale
  unviewed terminal status do not. `TURN_LIVE` decides lifetime;
  `TURN_RUNNING` remains for the interrupted-turn report on resume.
- Native Claude prompt/stop callbacks record unyielded/yielded activity on the
  current attachment through provider capability checks. Child callbacks,
  different conversations and replaced providers cannot write. Structured
  streams and Pi's native extension retain their activity path. Start/stop
  recording precedes steering or planning delivery so failed context delivery
  cannot suppress evidence of a started turn. Claude Stop snapshots retain
  background tasks and session-scoped scheduled wakeups as outstanding work;
  a later complete empty snapshot settles them. Missing arrays preserve the
  prior count for that same provider. A yielded reply or viewed idle terminal
  cannot settle provider-owned background work.
- Command output supplies additional activity. Executable removal/replacement
  requires two seconds of output silence plus the same store judgment; viewer
  changes cannot extend this interval. Silence never settles a recorded turn.
  Native Claude permission waits stay held until Stop; finer permission-wait
  behavior needs demo evidence.
- Store failures retry. Store-open failures log once per distinct error;
  judgment and viewer-write failures log each occurrence.
- Before policy or command exit, save `$LF_HOME/screens/<session>.ghostsnp`
  atomically through LOO-710's snapshot writer. Live attach and resume use its
  Ghostty repaint. External SIGTERM writes no snapshot. Keep one overwritten file
  per Session, with no expiry or deletion; no age/space policy was accepted.
- `lf resume` reopens provider history at the bottom without replay or a new
  turn. `lf resume --all` selects unfinished interactive Sessions whose owner
  was viewed, recorded no exit (`outcome IS NULL`), and is gone from the OS.
  Retention closing a dead row does not hide it. Clean SIGTERM records an exit,
  so clean reboot restores nothing through `--all`; Desktop pane restore uses
  ordinary resume. Busy but unviewed crashed owners remain for explicit resume.

## Delete — do not maintain

Completed cuts: the UserPromptSubmit steer-marker shortcut (tool callbacks
retain it); store-only executable-gone judgment; `LONGEST_PAUSE`, `recheck`,
`held_until`, `Grace::due`; unreadable-config disabling; duplicate
`RUNNING_SESSION_GRAPH`. Reuse `SESSION_GRAPH`, `session_activity` and the
provider/attachment fences. No schema, DTO, public mode or second lifecycle.
Completed this pass: `record_native_turn`'s unconditional `open_tools: 0` and
`TURN_LIVE`'s terminal-idle precedence over owned background work.
The surviving native callback records Claude's complete `background_tasks` and
`session_crons` snapshots in the existing activity count. No new registry,
polling process, status inference or completion timer is introduced.
`TURN_LIVE` now states its precedence directly: owned work, viewed status, then
the current attachment's unyielded turn. Native fixtures use the shared provider
installer. No deletion targets remain.

**LOO-710 handoff completed during sync, 2026-11-02:** pinned main
`ece53eec1b0274bd549525be5efbdc0bfb3965f2` includes the agreed Ghostty writer
and reader. Both policy and command exit use `snapshot::leave`; the `.vt`
writer and its exclusive terminal helpers are removed. Transport fixtures decode
`.ghostsnp`. No DTO field, expiry or deletion policy is added. Main's LOO-705
rollover response and wake pipe are combined with this branch's activity and
wall-clock judgment. Source integration proves no installed acceptance.

## Why the 24-hour rule failed

2026-11-02: 56 owners alive, 27 older than a day; 39 idle Claude processes
held 8.7 GB. Live-store samples and the LOO-672 audit found:

1. Finished Graph ancestry/membership held owners indefinitely; five of seven
   sampled October 30 owners were ancestors of finished Graphs only.
2. `Instant` counted awake time: 170 of 266 hours since boot on this Mac.
3. Pre-rule builds from before October 29 (#1765, v0.15.1) had no policy.
4. Propagated store errors killed the policy thread: 43 locked-database deaths
   on October 27 and 34 incompatible-database deaths on October 30; zero idle
   ending log lines.
5. Stale terminal status looked like a running turn forever. Unreadable config
   also silently disabled ending.

LOO-705's `lf monitor rollover` owns the 27 pre-rule owners; leave them intact
and point the PR there rather than prescribing signals.

## Remaining acceptance

- Gate: lifecycle, transport and launch-contract checks, plus Clippy. The native
  repair reaches `store::sqlite::steers_tests`, `context_block`,
  `harness::context`, `lf::commands::context`, `transport_tests` and
  `integration::session_cli_tests`, plus `world_recipes` watch cases and
  `store::sqlite::program_status` for activity/terminal precedence.
- Preserve regressions for active native turns across both deadlines and ending
  after Stop; current-attachment handoff/replacement fencing; continuous output;
  Graph ancestry and captured-step membership; stale status; installed symlink
  changes; headless Graph steps and plain `lf -b skill … --message` Runs; saved
  screens at idle and command exit; viewed-only crash restoration.
- The restart fixture kills owners/providers and closes an owner row as
  retention would after reboot. It cannot advance kernel boot time; with dead
  PIDs, advancing it would not change the judgment.
- `s.interactive=0` stays in the store judgment: pipes skip the detached limit,
  but executable removal still asks this shared question and must preserve Runs.
- Demo: return all twenty conversations at the bottom without new turns. Only
  three were resumed in the recorded probe; that does not meet full acceptance.
- PR: include the numbers below and describe the integrated LOO-710 snapshot
  writer and reader. Installation and the complete return acceptance remain unproved.

## Concept review (2026-11-02)

Jack Heart's silent-turn requirement fits the existing Session/provider/attachment
model; no suspended state, separate resume lifecycle or additional skill guidance
is needed. No further product choice was identified beyond the recorded assumptions.
Detach → retain live work → settle → save/end after quiet → resume provider history.
Executable removal changes the interval, not turn authority. Generated callbacks
use the stable entry independently of the owner's removable build.

Required counterexamples are retained above: silence cannot settle work, old
providers cannot settle replacements, finished Graphs cannot retain owners.
Source/coverage review adds no runtime acceptance; all twenty returns and real
permission waits remain with demo.

## Recorded live evidence (2026-11-02)

Branch debug build, throwaway `LF_HOME`, real Claude Code 2.1.316, this checkout:
20 conversations viewed once, detached at 13:53:07. Before: 60 processes
(20 owners, 20 launchers, 20 Claude), 9.9 GB RSS. All owners ended by 13:55:23
(2m grace), outcome interrupted/130; none of the 60 processes remained.
Twenty screen files and idle-ending log lines, no store errors. Three resumes
launched `claude --resume <thread>` without replay; saved screens showed the
welcome and pending prompt. The 27 pre-rule owners were untouched.

Native watch review (Jack Heart, 2026-11-02): Claude Code 2.1.317 in a
throwaway native home and PTY, using a credential-free local Messages endpoint,
ran a real background Bash watch on a completion file. Its first Stop contained
one `shell` task with `status: running` and no scheduled wakeups. Five seconds
later the file released the watch; Claude processed its completion notification
and emitted a second Stop with both arrays empty (three model requests total).
Evidence: `/private/tmp/native-watch-y1t6wzw1/hooks.jsonl` and
`.lf/tmp/native-watch/probe.py`. This is real provider lifecycle evidence with
synthetic model responses, not installed acceptance. All probe processes ended.
The [Claude hook contract](https://code.claude.com/docs/en/hooks#stop-input)
identifies both arrays as complete snapshots when its task registry is reachable.

The generated-hook transport regression failed before repair: the owner ended
past its grace with a yielded reply and a running watch. It now proves survival
past both the grace and executable removal, missing-snapshot and child-callback
isolation, scheduled work after shell completion, and owner exit after the final
empty snapshot. Store regression also reproduced terminal-idle precedence while
viewed; owned work now holds through that report and attachment handoff, with
replacement-provider fencing intact. These fixtures preserve the prior silent
turn test; they do not claim every external process is provider-owned work.

## Check result

`cargo test -p loopflow --lib --test transport_tests -- silent_native_turn native_background_watches_survive_until_settlement detached_sessions_end_after_their_limit sessions_end_when_their_lf_executable_goes` (inherited `LF_*` unset, stdin closed): 5 passed; `cargo fmt`, `git diff --check`: passed. Nonfatal linker unwind-size warning. Gate owns broader suites and Clippy; demo owns all twenty returns and native permission-wait judgment.
