# LOO-708: Loopflow's own processes stay small while conversations sit idle

Jack Heart measured about 150 `lf` processes holding 5.5 GB of footprint on
2026-11-02. Three changes, one PR. Measured on this Mac before the change
(footprint, release build): owner 16 MB, inner `session connect` 17 MB,
`lf -i` launcher 58 MB (peak 126 MB); a store-opening `lf` is 38 MB peak, a
Clap-only `lf --version` 6.8 MB peak.

## Accepted decisions (Jack Heart, 2026-11-02, Task directive)

1. The owner (`lf session serve`) takes over the launcher's wait; no `lf -i`
   stays beside Claude. Chosen over freeing the launcher's heap in place.
2. The viewer is a thin relay of a few MB, profiled first.
3. Desktop's chain flattens to one owner plus one viewer per conversation.

Scope split (Jack Heart, 2026-11-02): the viewer wire format (binary frames,
snapshot-first stream, byte-counted resume, nonce-tagged control messages)
belongs to LOO-712, running in parallel; this one changes which processes exist
and what each holds, and leaves the stream encoding in `transport.rs` alone.
The LOO-672 audit's 5 ms serve loop (200 wakeups/s per idle owner) is in scope:
the loop blocks in `poll` on its descriptors and a wake pipe that a blocking
child wait and the lifetime policy write to. This leaves SIGCHLD to background
runtimes and eliminates the global signal descriptor. Timed output waits use
the actual exit/replay deadline.

No new provider turn is authorized by this work. Shared owners, Desktop reading
sockets directly, polling commands and ending idle providers remain outside
LOO-708. The current per-conversation implementation does not decide the later
shared-owner question.

## Implemented approach (reconciled 2026-11-02)

**Exec, not a new wait.** A process that already owns the child keeps owning
it across `exec`: PID, parent, PTY, process group and the owner's `wait`
all survive. So nothing learns to wait on a non-child.

- **Launcher → provider.** `run_native_session` (the generic/Claude branch,
  owned launch under a transport owner) records the AgentProcess identity for
  its own PID, hands the owner a `NativeExit` record over the control socket
  (`Request::ExitRecord`, acknowledged with `Response::Accepted`), then `exec`s
  the provider `Command`. The launch lock stays held until identity is recorded.
  The owner completes the launcher's ledger row through `record_process` on child
  exit, performs what the launcher used to: AgentProcess exit, attachment
  finish, capture settlement (`CaptureHandoff` carries attempt/seq state),
  account re-pin, moved notice, native exit text, status report, diagnostics
  line, and runs the repository's planning sync meanwhile. A refused or failed
  handoff falls back to the ordinary spawn-and-wait. Codex and OpenCode keep
  their launcher (relay clients, private servers). Graph steps share a terminal
  but remain children of their Graph: the owner declines their wait handoff,
  so the existing launcher settles each step before the Graph continues.
- **connect → `lf -i`.** `serve_locked` `exec`s the `lf -i` command instead of
  spawning it; the launch lock crosses the exec as an inherited descriptor
  (`LF_SESSION_LAUNCH_LOCK`) and the launcher drops it where it already drops
  the resume path's lock. Handoff env uses `carry_across_exec`.
- **Viewer → relay.** `transport::attach` completes the viewer's ledger row
  and `exec`s `lf session attach OWNER [--moved TEXT]`, handled at the top of
  `run()` before Clap, tracing or any store: socket, stdin, stdout only.

## Delete — do not maintain

- Removed: the owner's independent raw-status result mapping and capture-outcome
  decision in `native_exit::settle`. `SessionCommandOutcome` now interprets both
  returning and handed-off launches; native failure text, attachment fencing,
  departure notices and `record_process` remain.
- Removed: the duplicated native-client completion branch in `transport::serve`.
  A launcher announcement or the owner's child wait enters the same drain/finish
  path before settlement. No second wait or terminal-completion authority remains.
- Keep `spawn_session_process`, `session_is_resumable` and their exclusive
  polling fixtures deleted, along with the launcher wait on the exec path and
  the viewer heap. The Session-ID continuation restores post-return output
  without a retained waiter or live relay store. LOO-712 owns framing.

No remaining deletion target was identified in this pass.

## What changed during implementation

The inherited launch lock replaces resumable-history polling and its binary
digest diagnostics. Launcher post-wait bookkeeping moved to the owner on the
exec path; other providers retain their wait. Compression removed the separate
process-completion SQL writer in favor of `record_process`, and replaced the
global SIGCHLD descriptor with a blocking child wait. Earlier implementation
detail remains at `2a0dd5beb8:scratch/loo-708-processes-stay-small.md`.

The suspected viewer terminal parser was not the retained cost: the owner keeps
the terminal state. Exec discards the viewer's launch/store/runtime heap without
changing the stream encoding. The owner's footprint rose while the total fell;
it now owns planning sync as well as exit settlement. Blocking the descriptor
loop removes its 5 ms timer, not every timer: the lifetime policy still checks
the executable and the relay checks terminal size periodically.

## Related work and integration

Local `origin/main` is `4e9ade2d04` and is already merged at `2a0dd5beb8`;
its installed-app routing and store-skew fixes remain intact. This is the local
reference, not a fresh remote observation.

Jack Heart's 2026-11-02 coordination comment on LOO-705
(`5bbe5c1c-532e-454f-96a0-212a80091b25`) requires an owner-supplied successor:
the installed entry plus `session resume <session id>`. Replaying this branch's
relay argv would attach to the ended owner. LOO-705 owns that behavior; its
PR #1799 is open at reconciliation. The accepted order recorded on LOO-712 is
LOO-705, then LOO-708/LOO-709, then LOO-712 carrying the successor in its control
event. That integration is not in this checkout yet.

Related branch memory was inspected at `81544931d3` (LOO-712), `7cdda980f5`
(LOO-709) and `ca6493ff68` (LOO-710). These are branch evidence, not merged
behavior here: LOO-709 changes lifetime policy; LOO-710 changes terminal state;
LOO-712 changes framing. Their integration must preserve this branch's wake
pipe, exit handoff and store-free relay. Release child Wave memory reinforces
testing the real entry path rather than treating a helper fixture as installed
acceptance; its GOAL.md and full MEMORY.md were read, with no other immediate
child Wave directories present.

## Acceptance evidence and remaining work

- Caller review after the preservation repair found one remaining JSON cutover:
  `human_session::fork` and `human_session::new_thread` call `serve_locked`
  before the JSON-aware delegation in `human_session::open`. The former receives
  no JSON intent; the latter also serves `session new` and provider-changing
  resume. Their exec still bypasses the caller's `print_opened`. Next implement
  must carry the correct resulting Session ID through these paths too, without
  replaying the fork/new-thread mutation or retaining a waiter. Add focused
  `resume --fork --json` and `session new --json` proofs, including one provider
  launch and the resulting Session identity. This is a code-inspection finding;
  the seven passing transport checks below do not exercise these paths.
- Implemented and fixture-proven: one owner plus one relay through Desktop's
  prepared-command path, Claude directly parented by the owner, one provider
  launch, attachment exit, AgentProcess completion, capture settlement and
  launcher completion. The interactive Graph regression proves each step
  settles before the next starts. These use stand-ins, not a rendered Desktop
  or authenticated Claude.
- Still required: exit recording, takeover notices, `lf resume`, reboot
  `lf resume --all`, and Graph behavior remain intact. Transport passed after
  compression; gate owns lifecycle/launch-contract, startup-cost and journal
  acceptance checks against the final implementation. Earlier lifecycle (28),
  launch-contract (25) and focused unit (111) passes are baseline evidence.
- Still required: real Desktop/Claude process-count acceptance and PR inclusion
  of the footprint comparison. No PR exists yet. The release measurement below
  predates compression; no final-tree measurement is claimed.
- Integration after LOO-705 must retain the explicit successor through the
  thin relay. LOO-712 owns subsequent encoding changes. No substantial missing
  process implementation was identified in this reconciliation.
- Concept review below resolves the draft compatibility assumptions by preserving
  the existing command contract. The repair below restores the JSON continuation and launcher-result
  interpretation; no acceptance of their removal is inferred.

Measured before compression (release build, isolated home, Claude stand-in,
`footprint`):
owner `lf session serve` 20 MB (it now runs the launch's planning sync),
relay `lf session attach` 2.5 MB, nothing else; before, the same conversation
held owner 16 MB + inner connect 17 MB + `lf -i` 58 MB. Claude's parent is the
owner.

## Concept review — 2026-11-02

**Result: command-contract preservation across exec. No unresolved product
choice.** Jack Heart accepted
removing retained processes while keeping resume, exit recording, takeover and
Graph behavior intact. That supplies the default: relocate the command's remaining
work, not remove its observable result. This is a consequence of that requirement,
not new approval from Jack Heart.

The useful concepts remain Session (durable conversation), AgentProcess (provider
identity), attachment (exact control authority), and execution owner (wait,
terminal and settlement). The relay is disposable transport machinery. Neither
the internal `session attach OWNER` entry nor `NativeExit` needs to become a new
user-facing lifecycle. No new table, shared daemon, provider turn, or wire encoding
change follows from this review. Graph children and Codex/OpenCode retain the
waits their current execution paths need.

### Experience and proposed documentation

Keep the existing Session and transport usage guidance. The following behavior
is the preservation target:

```sh
lf resume SESSION                  # return to the same conversation
lf resume SESSION --prepare --json # return the command Desktop opens
lf resume SESSION --json            # retain the existing Session JSON after a successful return
```

Closing the viewer leaves the provider and capture with the owner. Reopening
attaches to the same live provider; a takeover tells the departing viewer where
the conversation went. After a reboot, `lf resume --all` restores eligible
interrupted conversations from history without replaying input. Existing skills
need no extra command or distinction for the smaller process chain.

### Consequential mismatch: a process disappeared, but its command still owes a result

1. **Preserve the JSON continuation.** `session.rs::resume` calls `print_opened`
   after `reopen`, and `print_opened` honors both JSON flags. The old attach returned
   through `human_session.rs::open`; successful `transport::attach` now execs and
   cannot reach that continuation. The unchanged `--prepare --json` path does not
   replace this behavior. Carry only the information needed to finish the command;
   any final store read can happen after relaying ends. Keep the live viewer small
   and store-free, and retain the output's existing timing and success conditions.
   Do not add a waiter or change LOO-712's stream encoding to solve it.
2. **Preserve launcher semantics when the owner settles.** A native provider
   exiting 7 formerly returned an ordinary error from
   `util.rs::spawn_session_command_with_env`, mapped by
   `journal::command_exit_code` to launcher exit 1. `native_exit.rs::settle` instead
   assigns 7 to both the launcher record and viewer exit. It also treats raw 130
   as an interruption and computes capture outcome without `lost`, whereas the
   returning path includes attachment loss in `SessionCommandOutcome.interrupted`.
   Use the same interpretation for handed-off and returning launches: preserve
   the established command result, capture outcome and takeover behavior while
   retaining native status as evidence. Moving the writer to the owner must not
   silently redefine success or interruption. Keep attachment fencing and the
   shared `record_process` writer; no parallel persistence model is needed.

The startup error can come directly from the launcher: the deleted connect poll
need not survive just to print “before becoming resumable.” The existing lifecycle
test instead requires the actionable “failed to start” error. This wording does
not change the recovery action or authorize another launch. PlanningSync moving
with the wait likewise adds no user choice.

### Counterexamples and next proof

- Normal: prepare a Claude conversation, open its prepared command, detach and
  reattach. Retain one owner plus one small viewer, the provider identity and one
  launch. On successful `resume --json` return, retain the Session JSON too.
- Failure: a native stand-in exits 7 or 130. Compare the returning launch with
  the owner-settled launch across CLI result, Process result and capture outcome;
  a raw status alone must not invent a different command contract. A failed exec
  must still report failure and settle without a second provider launch.
- Recovery: move an attachment during a live launch, then end the old provider.
  Preserve the departure notice and existing capture semantics without modifying
  the new attachment. Kill an owner and use the existing reboot-resume fixture:
  Session/history survive, and prior input is not replayed. Graph steps still
  settle before their successor starts.

These are focused proof obligations for the repair.
LOO-705's explicit successor integration and final Desktop/footprint acceptance
remain as recorded above. The review itself changed only scratch documentation; the implementation repair
below follows its direction without changing Wave memory or the review boundary.

## Preservation repair — 2026-11-02

`resume` passes its JSON intent to `open`. A reconnect carries the Session ID
through the relay exec; a newly delegated connection transfers its JSON tail to
the viewer and strips the child's JSON flag. The relay reads the current Session
record after success, including viewer takeover, and never opens the store while
relaying. Failed relays return their failure without printing a success record.

The owner and returning native launcher share `SessionCommandOutcome` for command
results and captures. Raw exits 7/130 map to command failure 1; only recorded
interruption or attachment loss changes that interpretation. Exit text retains
the provider status. Settlement errors also fail the viewer rather than inheriting
a successful provider code. Failed exec returns to the launcher and withdraws the
owner's handoff, preserving one settlement path.

Six focused repair checks passed: returning/handed-off exits 7 and 130,
failed exec with one failed capture, JSON on initial launch and reconnection,
JSON after viewer takeover, and stale-owner settlement preserving a replacement
attachment. The existing one-owner/one-relay assertion remains in the successful
reconnection test. Root `--json` remains rejected by existing CLI validation;
these proofs use the command's `--json`.

Gate retains the broader acceptance inventory above, including `binary(lf)`,
transport, session lifecycle/launch-contract, agent startup and journal checks;
`scripts/test.py --list` selects architecture and Rust for the branch. No new
authenticated provider turn or Desktop claim is made.

## Checks

2026-11-02: `cargo nextest run -p loopflow --test transport_tests -E 'test(a_terminal_launcher_becomes_its_provider_and_the_owner_records_its_exit) | test(native_exit_preserves_returning_launcher_results) | test(failed_native_exec_settles_once_without_launching_a_provider) | test(interactive_graph_settles_each_step_before_starting_the_next) | test(resume_json) | test(opening_a_prepared_conversation_runs_one_owner_and_one_relay)'` passed (7); prior unchanged stale-owner settlement test reused; `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `git diff --check` passed; `lf context` within both budgets; inherited `LF_*` cleared and stdin closed; fork/new-thread JSON cutover belongs to implement, broader acceptance to gate, Desktop/footprint judgment to authored review.
