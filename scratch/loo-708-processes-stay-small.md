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

## Implemented approach (reconciled 2026-11-02, preservation pass)

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
  and `exec`s `lf session attach OWNER [--moved TEXT] [--session-json SESSION]`,
  handled at the top of `run()` before Clap, tracing or any store. The relay
  uses socket, stdin and stdout; only a successful return opens the store to
  print the requested Session record. Fork/new-thread callers now delegate
  `session resume RESULT_ID` after preparation and carry that ID to the relay.
  They do not replay the original mutation. Thread replacement releases its
  preparation lock; the opener reacquires it and reads the current input.

## Delete — do not maintain

- Removed: the owner's independent raw-status result mapping and capture-outcome
  decision in `native_exit::settle`. `SessionCommandOutcome` now interprets both
  returning and handed-off launches; native failure text, attachment fencing,
  departure notices and `record_process` remain.
- Removed: the duplicated native-client completion branch in `transport::serve`.
  A launcher announcement or the owner's child wait enters the same drain/finish
  path before settlement. No second wait or terminal-completion authority remains.
- Removed: `NativeExit::lf_process_id`. The validated attachment already names
  the launcher as `provider_lf_process_id`; settlement and terminal status use
  that identity instead of carrying a second copy. Transport fixtures share
  provider installation while retaining each scenario's script and assertions.
- Keep `spawn_session_process`, `session_is_resumable` and their exclusive
  polling fixtures deleted, along with the launcher wait on the exec path and
  the viewer heap. The Session-ID continuation restores post-return output
  without a retained waiter or live relay store. LOO-712 owns framing.

Removed in the mutation-path repair: direct `serve_locked` calls from `fork`
and `new_thread`. Both now use the same prepared-input opener; no mutation
command is replayed in the owner. No remaining deletion target is identified.

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
PR #1799 was open at the prior reconciliation; no fresh hosted state was read.
The accepted order recorded on LOO-712 is
LOO-705, then LOO-708/LOO-709, then LOO-712 carrying the successor in its control
event. That integration is not in this checkout yet.

Related local branches were rechecked on 2026-11-02: LOO-705 at `c788a6ea32`
sends the full successor for removed, replaced and superseded executables;
LOO-712 at `35a2d5eec7` carries it through the control event and restores the
terminal before following, with exact-argv and exit/error fixtures. Neither
branch is merged here. Their combined integration must retain this branch's
wake pipe, exit handoff, store-free live relay and post-return JSON obligation.
LOO-712's framing and per-viewer appearance changes remain outside this Task.

LOO-709 memory at `9bc58f47ba` now protects recent native output for both
lifetime exit causes, retains running Graphs/headless Runs, and records its
agreement with LOO-710 on `screens/<session>.ghostsnp`. Its disposable-Home
check ended 20 owners but resumed only three; this is not acceptance here.
LOO-710 is now at `bf9947eb1c`: its source owns terminal cleanup from saved
preview through live attachment, including startup failure and pre-acknowledgement
cancellation. Its plan records primary-screen, palette and public-resume fixture
proofs, not configured Claude/Desktop acceptance. Its `delegate_with_preview`
passes an in-memory `ViewerTerminal` into attachment; this branch's relay exec
cannot inherit that cleanup object. Integration must preserve preview cleanup,
the store-free live relay and JSON continuation without retaining a waiter.
Snapshot fidelity and lifetime responsibilities remain with their owning Tasks;
neither branch is merged here.

## Acceptance evidence and remaining work

- The fork/new-thread JSON cutover is repaired: JSON intent and the resulting
  Session ID travel with the prepared launch. Three focused checks prove
  `resume --fork --json`, threaded `session new --json`, and provider-changing
  resume launch once, append one input and return the correct identity.
  Fork also preserves its source input; each result contains one JSON record.
  Source inspection at `04552ff2b4` confirms this supersedes the supplied
  iteration feedback's direct-`serve_locked` defect; no repeat implementation
  remains for those three paths.
- Implemented and fixture-proven: one owner plus one relay through Desktop's
  prepared-command path, Claude directly parented by the owner, one provider
  launch, attachment exit, AgentProcess completion, capture settlement and
  launcher completion. The interactive Graph regression proves each step
  settles before the next starts. These use stand-ins, not a rendered Desktop
  or authenticated Claude.
- Exit/continuation checks cover returning and handed-off exits 7/130, failed
  exec with one failed capture, JSON on initial launch/reconnection/takeover,
  and stale-owner settlement preserving a replacement attachment.
- Gate owns lifecycle/launch-contract, startup-cost and journal acceptance
  checks against the final implementation. Transport's exit, takeover, resume,
  reboot recovery and Graph checks passed after compression. Earlier lifecycle (28),
  launch-contract (25) and focused unit (111) passes are baseline evidence.
- PR #1802 includes the footprint comparison. Real Desktop/Claude process-count
  acceptance remains unproved. The release measurement below predates
  compression; no final-tree measurement is claimed.
- Integration after LOO-705 must retain the explicit successor through the
  thin relay. LOO-712 owns subsequent encoding changes.
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

Current review at `04552ff2b4`: **nothing to change and no open product
choice.** Source and fixture inspection confirms the preservation direction
below is implemented, including fork and thread replacement. The concrete
interaction remains open → detach → reopen the same Session; failed startup
retains prepared input for retry, and reboot recovery uses saved history without
replaying the interrupted turn. A relay has no independent durable lifecycle:
its exit cannot complete the conversation or transfer provider authority.
The transport README already describes that experience; no usage or skill
rewrite is needed. The review page now shows the current result instead of
presenting the repaired gaps as proposed work.

Next proof remains the affected gate suites and final Desktop/Claude process
count and footprint acceptance listed above. Integration must preserve successor
selection and preview cleanup across exec; these are established obligations,
not new product choices or permission to take over LOO-705/710/712. This review
changes no implementation, starts no provider, and claims no new runtime pass.

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

The concept review found that exec discarded `resume`'s `print_opened`
continuation, and owner settlement reinterpreted raw provider status instead
of preserving the returning launcher's command/capture result. Both are repaired.
The initial review and counterexamples remain at
`705e6c150a:scratch/loo-708-processes-stay-small.md`, Concept review.

The failed assumption was that replacing a waiting process automatically
preserved its caller's remaining work. Exec keeps process identity and open
handles, but discards the call stack. A continuation must follow the actual
resulting Session identity through each entry, including mutation paths.
Before this pass, fork and new-thread reached `serve_locked` before `open`'s
JSON-aware delegation; `fork` received no JSON intent and `restarted` dropped
it on the threaded branch.
Delegating the original command after the mutation would replay fork/new-thread
creation in the owner. The repair separates the prepared launch
from that mutation, carries the resulting identity, and retains one launch.
This follows the accepted preservation contract; no new product decision
is inferred.

The removed connect poll's historical startup wording need not return: the
launcher's actionable failure remains the contract. Graph steps still settle
before their successor; reboot recovery and final Desktop/footprint acceptance
retain their separate checks above.

Ordinary resume transfers its JSON continuation to the viewer and strips the
child's JSON flag. The relay prints the current Session record after success,
including viewer takeover; failure prints no success record. `--prepare --json`
is unchanged. Root `--json` remains rejected by existing CLI validation; the
proofs use the command's `--json`. Usage lives in
`rust/loopflow/src/transport/README.md`.

The owner and returning native launcher share `SessionCommandOutcome` for command
results and captures. Raw exits 7/130 map to command failure 1; only recorded
interruption or attachment loss changes that interpretation. Exit text retains
the provider status. Settlement errors also fail the viewer rather than inheriting
a successful provider code. Failed exec returns to the launcher and withdraws the
owner's handoff, preserving one settlement path.

## Checks

2026-11-02: reused unchanged-code results recorded at `04552ff2b4`: `cargo test -p loopflow --test transport_tests` passed (30), `cargo test -p loopflow --lib harness::native_exit::` passed (1; nonfatal linker unwind-size warning), `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed (tests cleared inherited `LF_*` and closed stdin); reconciliation: `git diff --check` passed and `lf context` within both budgets. Gate owns remaining affected suites; authored review owns Desktop/footprint judgment.
