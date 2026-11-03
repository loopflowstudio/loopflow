# LOO-709: An idle conversation costs nothing

Jack Heart's 2026-11-02 goal: closing or abandoning a conversation leaves no
resident agent. His continuation and review steers retain idle ending, require
silent native turn preservation, and keep the LOO-710 snapshot agreement.
Rust only; Desktop (LOO-672), process overhead (LOO-708), and snapshot fidelity
(LOO-710) stay with their owners. Earlier design and concept review:
`9bc58f47ba39ee1efaf0cc6677a37095d93d187f:scratch/loo-709-idle-sessions-end.md`.

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
  cannot suppress evidence of a started turn.
- Command output supplies additional activity. Executable removal/replacement
  requires two seconds of output silence plus the same store judgment; viewer
  changes cannot extend this interval. Silence never settles a recorded turn.
  Native Claude permission waits stay held until Stop; finer permission-wait
  behavior needs demo evidence.
- Store failures retry. Store-open failures log once per distinct error;
  judgment and viewer-write failures log each occurrence.
- Before policy or command exit, save `$LF_HOME/screens/<session>.vt` atomically:
  vt100 scrollback followed by the visible redraw. Live attach redraws only
  the grid. External SIGTERM writes no snapshot. Keep one overwritten file
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
No remaining deletion is due in this pass.

**LOO-710 handoff:** Jack Heart requested agreement before either publishes;
LOO-710 agreed on `<home>/screens/<session>.ghostsnp`, no DTO field, and owns
`lf resume` painting the saved screen before provider launch when stdout is a
terminal. Whichever branch lands second deletes the `.vt` writer:
`transport::{save_last_screen,write_last_screen,last_screen_path}`,
`Output::last_screen`, `terminal::last_screen` and its exclusive helpers/test,
and rewrites the transport fixture decode for `.ghostsnp`. LOO-710's APIs are
`transport::snapshot::{leave,last_screen}` (comment `a49ccb66`, 2026-11-02).
Keep this branch's writer until that cutover. LOO-705's superseded exit uses
this same policy-ending arm.

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
  `integration::session_cli_tests`.
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
- Realign may correct Wave memory's stale silent-native and pruning statements;
  memory remains read-only in this compress step.
- PR: include the numbers below and distinguish this writer from LOO-710's
  reader. Installation and the complete return acceptance remain unproved.

## Recorded live evidence (2026-11-02)

Branch debug build, throwaway `LF_HOME`, real Claude Code 2.1.316, this checkout:
20 conversations viewed once, detached at 13:53:07. Before: 60 processes
(20 owners, 20 launchers, 20 Claude), 9.9 GB RSS. All owners ended by 13:55:23
(2m grace), outcome interrupted/130; none of the 60 processes remained.
Twenty screen files and idle-ending log lines, no store errors. Three resumes
launched `claude --resume <thread>` without replay; saved screens showed the
welcome and pending prompt. The 27 pre-rule owners were untouched.

## Check result

`cargo test -p loopflow --lib --test transport_tests silent_native_turn`: passed (2); `cargo fmt --check`, `git diff --check`, `lf context`: passed; macOS linker warned about the large unwind section; affected suites and Clippy: gate.
