# Codex resume incident, 2026-11-08

Current status after gate: the two recovered Sessions retain their original
identities. The repair is checkpointed, reconciled with main and passes the local
gate; see `gate-resume-titles.md` for commands and results. No build was installed.
The parent owns landing and installed-build verification; live cmux remains
unverified. Historical evidence below describes the earlier stages.

## Report and preserved evidence

Jack reported `lf resume -a codex` on `jack-heart/search` in manabot returning
`Error: invalid control authority: conversation changed before input replacement`,
and said "got that in manabot and somewhere else".

Installed lf: `0.15.1+e3c888924`. This checkout: `3b07e13404`.
The initial recovery ran in the incident Graph's unbreak step (Process
`df00c50a-09d6-485a-9191-1467a6ec048a`); the parent owns subsequent steps.

SQLite backup before recovery:
`/Users/jack/.lf/incident-backups/codex-resume-20261108/loopflow-before.db` (544 MiB).
The unrelated `config/migrate.rs` edit and both `scratch/upgrade-*` notes remain intact.

## Live observations, starting 03:30 PST

`git worktree list` identifies `jack-heart/search` as `/Users/jack/src/manabot.search`.

| Session | Evidence |
|---|---|
| `session_38c2c72446b44407befacbaad28fef2f` | Original manabot.search Claude/opus conversation, native thread `5537a213-b123-46cf-a71e-dc3ac90c37f2`. Capture `2875145` unchanged. Event `2883689` records interrupted attachment with departure to `ttys028` at 03:29:17; Session completion has the same timestamp. Agent PID 23304 is gone. |
| `session_1b3696a30fb1484da65ef334d66925cd` | Matching second candidate in `/Users/jack/src/loopflow.agent-017315fa`, Claude thread `92f5edf7-2283-419f-ac46-ff15dc6e1af2`. Capture `2880953` unchanged. Event `2883571` records interrupted attachment with departure to `ttys002` at 03:25:32; Session completion has the same timestamp. Agent PID 86573 is gone. Jack did not name the second checkout, so attribution remains a candidate. |
| `session_c73fe3a8e9e94b56b1ffe915b575a9c0` | Separate Codex conversation started in manabot.search at 03:29:24, thread `01a1baeb-9c94-7ac0-94d2-6da188cecd56`. Already running before recovery; leave it alone. |

Both old Claude transcripts were readable: the failed handoffs had already written
`conversation.md` under their sharded capture directories in `~/.lf/runs/`.
The process ledger contains no failed resume row for manabot.search; the report
and attachment events supply the evidence, not an invented failed Process.

The installed source has a deterministic path to this error. `new_thread` reads
the Session, creates its continuation, calls `stop_session_agent`, then passes
the earlier Session to `append_input`. `finish_session_attachment` retires ordinary
Sessions on interruption. `replace_session_input` rejects the stale
`completed_at`. No competing writer is needed. Task-associated switches nearby
(LOO-793 and manabot b015d5) completed successfully and do not retire this way.
Debug should prove this with a focused reproduction before choosing a code repair.

## Recovery, 03:32–03:35 PST

Both old Sessions now continue on Codex under their original IDs. No source patch,
install, direct database edit, or speculative process termination was needed.

Ran the installed lf with inherited `LF_*` removed, from each original checkout:

```sh
lf -b resume <exact-old-session-id> -a codex --message '<bounded verification>'
lf resume <exact-old-session-id> -a codex
```

The bounded message explicitly prohibited continuing old work, tools, file edits,
Task launches or messages. It asked for the last topic from the supplied history
and `RESUME_VERIFIED`. Both headless invocations exited 0:

- manabot: "The last topic was the interim search-compute report, where depth-20
  mirror search with β=0.03 and four shuffles led, alongside batching Task MTG-268."
- Loopflow candidate: "The last topic was fixing #1844’s failing CI tests around
  when Task conversations start."

Then opened both with the interactive `lf resume <id> -a codex` terminal path.
Each displayed the recovered history and answered the typed no-tools request
with `TERMINAL_RESUME_VERIFIED`. Quit these verification terminals with `/quit`.
Both terminal commands exited 0. Final ledger and OS checks show their new
AgentProcesses and transport owners exited; no recovery process remains running.
The initial terminal paint briefly replayed the old departure screen, then the
Codex UI opened; it did not prevent the verified turn.

| Session suffix | Preserved Claude capture | New Codex capture | New native thread |
|---|---|---|---|
| `38c2c72446b44407befacbaad28fef2f` | `2875145` | `2883814` | `01a1baee-d8e3-74a1-9ee7-80b56d5c0b95` |
| `1b3696a30fb1484da65ef334d66925cd` | `2880953` | `2883843` | `01a1baef-a0b2-7e11-a73b-aba1490e466c` |

The old captures remain alongside the new ones. Interactive opening reopened each
ordinary Session; quitting retires it normally. Resume it explicitly by ID to
continue. Bare resume in manabot.search may select the newer, separately opened
Codex Session, which recovery did not touch.

## Unbreak handoff, before debug

- The installed interactive cross-provider path can still retire an ordinary
  Session and then reject its own stale snapshot. Recovery bypassed that path
  with a headless continuation before verifying interactive native resume.
- Debug needs a regression for a live ordinary conversation switching providers,
  retaining identity/history and accepting input. The second checkout's identity
  as Jack's "somewhere else" remains unconfirmed despite matching evidence.
- No failed resume Process was found for manabot.search. Investigate whether the
  failure rolls back its journal row; do not call that established yet.
- Both affected checkouts stayed clean. SHA-256 checks confirmed `migrate.rs`
  and both historical upgrade notes unchanged. A separately added
  `scratch/rex-title-2026-11-08.md` appeared during this run and was left alone.

## Debug: scope and source comparison

Compared this checkout with installed `e3c8889246`. `new_thread`, input replacement's
concurrency checks, attachment retirement, naming and terminal-title publication
retain the reported behavior. Newer main has typed AgentProcess IDs and asynchronous
exit settlement in transport; neither repairs these two causes. No live conversation
is used for reproduction. The parent incident Graph owns delivery and later steps.

Repair direction: a recorded departure ends an attachment without retiring its
Session. Keep input replacement's completion/capture/membership checks intact.
For titles, skip `default` as a generated purpose and publish OSC through the
transport owner's existing complete-sequence report queue. Query names off the
output loop; preserve viewer-specific cmux/tmux publication and snapshot replay.

## Debug: reproduced cause and repair

The new ordinary interactive provider-switch regression failed on the original
code with the exact reported error: `invalid control authority: conversation
changed before input replacement` (`/tmp/codex-resume-cli-red.log`). It starts a
live taskless Claude stand-in, preserves a transcript, then runs `-i resume <id>
-a codex`. The existing provider-switch test used a Wave and headless execution,
which avoided ordinary Session retirement.

The attachment test also failed before the repair: stopping an attachment for a
recorded move set `completed_at` to `1794138112`, where the continuing Session
requires `None` (`/tmp/codex-resume-move-red.log`). No concurrent writer was needed.
Attachment settlement now retires an ordinary Session only when there is no
recorded departure. The attachment still ends; Session identity, provider-history
artifacts and input replacement's completion/capture/membership comparisons remain.
Natural provider exits still retire disposable Sessions. This removes the lifecycle
mistake rather than weakening the stale-snapshot check or rereading past a conflict.

The naming regression failed with `default` instead of the directory fallback
`terminal-titles`. `default` now joins the excluded operator skill names. Standard
OSC publication runs through the terminal owner, with reads off its I/O loop.
The existing VT parser gates publication between complete sequences and detects
provider titles that would overwrite the current Session name. Reattachment
replays the name before any unfinished provider sequence. Host-specific cmux/tmux
controls and manual cmux workspace-name policy remain intact.

The first fixed library run passed 44 checks, including departure retention,
natural retirement, stale attachment/input rejection, name policy, directory
reports and OSC 7501 status handling. One new parser test failed because it
expected a byte-identical unfinished sequence in a snapshot; Ghostty reconstructs
canonical continuation bytes. The assertion now replays the snapshot and suffix
through the parser and checks completion/title state. Final rerun and lint results
are recorded below when available. `/tmp/codex-resume-focused.log` retains this run.

## Debug: final verification and delivery boundary

The bounded rerun passed all four tests (`/tmp/codex-resume-final-focused.log`):

- `session_renames_wait_for_complete_sequences_and_replay_on_attach`: every split
  of OSC with BEL/ST, CSI, DCS and UTF-8; safe snapshot replay, title sanitization,
  stale provider OSC 2, terminal reset, and no OSC on pipes.
- `live_taskless_provider_switch_preserves_identity_and_accepts_terminal_input`:
  the exact formerly failing interactive path succeeds; Codex receives the old
  transcript, Session ID/title persist, and both old/new capture history resolve.
- `provider_switch_imports_session_history_with_current_policy`: existing
  headless/Wave continuation and current-policy behavior still pass.
- `taskless_session_renames_reach_plain_terminals_and_reattached_viewers`:
  real terminal owner/viewer protocol carries the initial fallback and changed
  name, restores it after provider OSC 2, retains OSC 7501, and replays on attach.

Command (inherited `LF_*` removed, stdin `/dev/null`):

```sh
cargo test -p loopflow --lib --test session_launch_contract --test transport_tests -- session_renames_wait taskless_session_renames live_taskless_provider_switch provider_switch_imports --test-threads=1
```

The library test link emitted Apple's `__eh_frame section too large (max 16MB)`
warning; compilation and the four tests completed successfully. Earlier passing
checks were not rerun. Live disposable Rex, tmux and isolated Herdr checks all
passed; exact values and cmux's unavailable-server limitation are in
`scratch/rex-title-2026-11-08.md`.

No installed binary, recovered Session, or other live conversation was changed.
The repair is in this older checkout; the parent owns reconciliation with current
main, delivery, and verification of the installed build. cmux's actual display
remains unverified until a server is available; its host policy tests pass.
The missing failed manabot Process remains unexplained and was not broadened into
this repair. The original second checkout attribution remains a candidate.

SHA-256 still matches the pre-debug values for `config/migrate.rs` and both
historical upgrade notes. `git diff --check` passed. Required lint/format results
follow below.

`cargo clippy --all-targets -- -D warnings` passed in 39.49s
(`/tmp/codex-resume-clippy-final.log`). The first lint run flagged a single-arm
`match` in the new title reader; replacing it with `if let` was the only change
after the behavior checks. `cargo fmt --check` and final `git diff --check` passed.
No broader test rerun followed that equivalent style correction. All disposable
host servers/providers and this Run's verification commands have stopped. Changes
remain uncommitted in this checkout for the parent's next incident steps.

## 5whys: why the resume regression escaped

Investigation on 2026-11-08 read the current diff, lifecycle callers, existing
fixtures and their introducing changes. The saved red logs still contain the
exact authority error and `Some(1794138112)` versus `None`; the final log records
four passing regressions. No passing tests were rerun and no source was changed.

The causal chain is a lifecycle mismatch, not evidence of a competing writer:

1. Switching a live provider stops its attachment before replacing the input.
   That stop also retired an ordinary Session, invalidating the snapshot the
   same operation was about to use. The completion comparison correctly refused
   it (`human_session::new_thread`, `sessions::replace_session_input`). The red
   CLI reproduction establishes this path independently of the missing live
   Process row.
2. Retirement classified an attachment's `interrupted` outcome as conversation
   completion. The rule originated in `62d34344da` (#1390), for exited orphan
   conversations. `c777dd87d5` (#1746) later recorded a move's destination in the
   exit receipt but left retirement unchanged. The distinction needed to keep
   the Session open already existed for the old terminal's departure notice;
   retirement did not use it. This identifies a missed integration point, not the first occurrence
   of the bug: stopping a provider used this retirement path before #1746.
3. Coverage checked each part without the failing combination. The switch test
   began in `a20074665f` (#1553) as a headless Wave policy/history test.
   `e88fa7331b` (#1767) made its first launch interactive, but that launch finishes
   before the switch; the target remains a Wave Session and the switch uses
   `-b`. Neither a live ordinary attachment nor its retirement is exercised.
   Separately, #1746's ordinary-Session stop test checked the departure receipt
   and token, but never checked whether the Session survived. A correctly
   recorded move could therefore pass while ending its conversation.

**Prevention already implemented locally:** use the existing departure to keep
the Session open; retain natural-exit retirement and stale-input checks. The
added assertion covers terminal, account and Machine departures. The added CLI
regression combines a live taskless Claude attachment, interactive Codex switch,
preserved identity/history and a received continuation turn. It failed before
the patch and passed after it. These directly close the observed coverage gap;
another lifecycle object, retry or migration is not warranted by this evidence.

**Proposed review practice, not additional approved work:** for future move/resume
changes, select fixtures by the lifetime being changed (ordinary versus retained,
live versus exited), and assert the continuing Session as well as the departing
attachment. Keep the existing policy/history test for its separate purpose.
No broad provider-by-host test matrix or prompt rewrite is needed here.

Limits remain: Jack's "somewhere else" is still unattributed beyond the recorded
candidate. Nearby successful Task switches and successful headless recovery fit
the lifecycle explanation; they do not prove the installed interactive path safe.
Why the failed manabot command left no Process row is unresolved. The earlier
upgrade's unrecorded install is a different command path and does not explain it;
there is no evidence here for journal rollback. Investigate that separately only
with a bounded reproduction that inspects recording at failure.

## Execution decision, 2026-11-08

Keep the implemented repair here: ordinary interactive provider moves retain
Session identity/history and accept continuation input; bare taskless Sessions
start with meaningful names and publish subsequent names through serialized OSC,
with existing cmux/tmux controls retained. The diff and recorded regressions
support this scope. No further implementation or test rerun is warranted for
this final incident step. No Task, Wave or Graph is created or launched; no
commit, install, publication or merge is performed.

Remaining delivery work, when taken up:

- Reconcile this older checkout's repair with current main before installation,
  especially typed AgentProcess IDs and asynchronous exit settlement. Preserve
  the unrelated `config/migrate.rs` edits and historical upgrade notes. Recheck
  changed integration points with the focused regressions and required lint.
- After delivery, verify the installed live ordinary provider-switch path with
  a disposable conversation: same Session, preserved history, accepted input.
  Recovery through the headless workaround does not establish that result.
- Verify live cmux initial naming, rename, stale provider title and reattachment,
  including a manually named workspace, when a disposable server is available.
  Current coverage is host policy only. Rex/tmux/Herdr observations already pass;
  preserve the display-preference and layout limits in the title note.

Delivery is done when the reconciled installed build demonstrates those resume
and title behaviors; live cmux support remains explicitly unverified until its
check passes. The user-facing title contract already lives in `docs/surfaces.md`
and the transport README; regression tests carry the resume/stream contracts.
Preserve this incident's irreplaceable recovery evidence before any scratch cleanup.

Optional follow-ups remain separate: a bounded reproduction of the missing failed
Process row, and future review guidance for lifetime-sensitive changes. Neither
is a prerequisite for this repair. The second checkout's attribution stays
unknown; no further recovery is inferred. Semantic skill metadata and a broad
provider/terminal matrix are not justified by the evidence. Title detail remains
in `scratch/rex-title-2026-11-08.md`.
