# LOO-415 — live Session transport

## October 17 demo and delivery authorization

Jack Heart closed the native Claude demo tab with its Cedar/Amber question
unanswered, saw the question return on public Session attachment, and reported
“worked”. Answering Amber then produced `DEMO_SECOND:Amber`. The same real Claude
PID 60408 and AgentProcess survived; the demo used an experimental LF_HOME with
the branch binary and the real provider account. Installed-store integration,
SSH and other unperformed acceptance remain open. Details and evidence are in
[the demo note](transport-demo.md). Jack then requested `lf ship -b!`, explicitly
authorizing ship and merge and superseding earlier no-merge constraints below.
Gate must preserve this result and its limits in delivery context before cleanup.

## Scope and decisions

Reconciled October 17, 2026 against the working implementation. Owner topology
and native UI requirements are accepted; implementation and restoration coverage
remain under review. The supplied attachment-deadline feedback is addressed by
the implementation and the recorded transport proofs below. Local `origin/main`
and the branch base both name `72dbe0344f`;
no newer upstream revision was available in the inspected refs.

Jack Heart accepted an on-demand owner per conversation on October 16, 2026,
and selected LOO-415 to unblock LOO-416. This implementation remains one PR.
Publication and demo/review belong to their authored steps; no merge or auto-merge
is authorized.

The owner retains execution, capture settlement, provider transport/lifeline and
the native client, including its original request channel. A viewer never owns
those resources. The owner drains with zero viewers; terminal loss never submits
another turn. Session/native history stays in the existing stores, raw terminal
state stays in runtime memory, and shells remain ordinary Processes.

Jack's [native UI clarification](loo-415-work-api-direction.md) permits a necessary
provider restart on explicit resume while preserving Session/history and never
resubmitting input. Prefer the retained native terminal. Structured Runs stay
structured until an explicit native transition; this transition does not claim
continuity of its old pending request. Automatic attachment never transitions.
The separate Claude-mode investigation is evidence, not a new execution directive.

The original brief is retained in [evidence](evidence/loo-415-original-brief.txt).
The accepted direction supersedes its multiplexer exemption: conversations, Flows,
Task execution and shells have independent owners. Delivery controllers retain
their existing cancellation lifetime; their conversations own transport separately.
Remote commands use `lf --machine LABEL ...`. LOO-416 owns
product lifecycle, uniform primary creation and readiness/cache policy; LOO-497
and LOO-489 own shared arrangement/navigation. No parallel layout store or shared
resident server is introduced. No code, tests, help or configuration are copied
from herdr/cmux, and no old-version transport compatibility is selected.

## Implementation decision — October 16

The owner launches the actual command before capture admission. Its detached
execution child creates and settles the capture, so the creator has no local
CaptureHandle capable of settling accepted input on Drop. This avoids serializing
mutable capture handles or moving their frozen provider capabilities. The creator
waits through the same viewer protocol as later clients. Startup acknowledgement
means the owner has assumed the execution; missing acknowledgement remains unknown
and never retries. No second job/history store is added.

The existing Session row records `transport_lf_process_id` under its provider
attachment fence, in the one `session_transport` draft migration. The socket is
derived from canonical Machine home and this Process ID. Missing sockets for
unfinished executions fail without replacement. A completed AgentProcess cannot
attach a previous Flow step to the next step's terminal.

A Flow explicitly passes its terminal owner to its step children. The step remains
the actual Process that captures input and exits; terminal loss cannot settle it.
Other nested agent-issued commands start independent owners. Background PR repair
uses this same owner and retains explicitly inherited promotion descriptors.
The inherited owner ID and control key form one handoff value, consumed before
provider launch. Retained output has two modes: terminal state with pending status
frames, or a bounded stdout/stderr tail. Keeping that state together leaves the
owner loop responsible for process lifetime, viewer claims and I/O.

Viewer input uses fresh runtime tokens and never changes AgentProcess attachment.
Observe acquires no token. Reclaiming control fences stale clients, including
A → B → A; disconnect cannot release a newer claim. Slow viewers are removed
without blocking the owner. Native Codex keeps its existing engine/protocol relay
and native UI behind the retained PTY. Closing that native UI deliberately closes
its protocol channel; losing an outer viewer does not.

Native `resume` selects retained interactive execution first. Explicit transition
from a structured Run closes that provider, preserves history and opens the native
client under an owner. It does not reuse the old path that took the harness's
provider claim for a viewer. Saved first inputs are never resubmitted on resume. Explicit headless next-turn submissions also delegate before capture admission.

The terminal owner uses vt100 for bounded grid/scrollback and input-mode snapshots,
and vte for escape/UTF-8 boundaries. It provides detached DSR/device-attribute
responses. lf status travels separately to the owner, which serializes full OSC
7501 frames between complete provider sequences; it never enters child output,
JSON, captures or logs. Provider reports pass through unchanged. Explicit pending
input can report blocked; silence cannot. Linear Flow steps can report progress.

## Delete — do not maintain

- Removed viewer-scoped `human_session::connect_live_codex` and its exclusive
  transfer test. Native engine connection survives inside the retained client;
  replacement viewers never claim provider attachment.
- Removed its orphaned `resume_session_with_env` remote/attachment arguments and
  remote-launch branch. Retain `native_connection_command` in the owned Codex
  launcher and its provider-fencing tests.
- Removed tmux `start_session_with_context`, `start_tmux_session`, its exclusive
  detached-cwd test and installation dependency. Removed the unused repair name,
  duplicate launcher wrappers and their async signatures. One PR-repair launch
  uses the surviving owner and explicitly inherits any promotion descriptors.
- Removed tmux `/detach`. Viewer detach leaves accepted execution alone.
- Removed the unused per-viewer `Request::Detach` and its exclusive protocol probes;
  socket closure already detaches without releasing another viewer's claim.
- Removed `ReadFd`/`WriteFd` trait objects: PTYs and child pipes are owned Unix file
  descriptors and need only `File`. Removed `recover_history`'s unused empty-history
  return value; saved history never selects first-input replay.
- Replaced automatic headless-open stop logic with explicit native-transition
  semantics; attach is a separate operation with no recovery effects.
- Removed resume's saved-first-input replay. Capture Drop remains for actual
  executor failure and unhanded work, never the external creator/viewer.
- Replaced `transport::attach`'s unbounded acknowledgement wait with a viewer-only
  deadline; retain unlimited established attachments and existing signal cancellation.
- Keep `CodexConnection::serve_with_input`: its peer is now the retained native
  client, not an ephemeral viewer. Its explicit-native-exit test remains valid;
  the new whole-path proof replaces the old assumption about viewer EOF.
- Hidden `serve-conversation` and remote `human_open_argv` branch were already absent
  at baseline `72dbe0344f73722bd90a4f392c3e801cd976b80f`; released migrations are history.

## Verification and remaining acceptance

October 17 gate review found and repaired these regressions:

- Dispatch wrapped delivery controllers merely because they could launch an agent,
  changing cancellation and release-lock ownership. Delegate actual conversation,
  Flow and Task execution; keep delivery controllers at their existing boundary.
- Admitting the owned executor before Task placement recorded the wrong checkout
  and rejected valid Task starts. Only the external viewer admits before handoff;
  the executor admits after placement.
- Pipe-backed shell commands never received EOF, and interactive Flows lost their
  terminal when stdout was redirected. Forward authenticated pipe EOF after input
  drains and select a PTY when either stdin or stdout is a terminal.
- A fixed two-second post-exit window retained locks and exposed completed native
  results on reopen. Drain final output and exit for previously attached executions;
  keep the replay window only for starts that have never had a viewer.
- Clones could not create the new relay socket. Their inherited sandbox permits
  only their own canonical-home-derived private transport directory in addition
  to clone-local sockets. Source sockets and external networking stay denied.

Crash and ancestry fixtures now distinguish the viewer, owner and executor. The
repair-start fixture exercises a real transport-log failure instead of a removed
tmux shim. New transport proofs cover pipe EOF without lost bytes and a slow
observer dropping without blocking the healthy viewer or losing final output.
The review retained one owner protocol and the existing Process ledger; no new
state store, compatibility path or configuration switch was added.

Focused proofs use the built CLI and disposable Machine homes. Codex's stand-in
requires an answer on the original WebSocket request: creator exit and zero viewers
leave native PID/thread/turn and provider attachment unchanged; replacement answers
without capture settlement or input replay. The Claude-shaped shell stand-in and
shell fixture cover launch-then-attach,
retained shell cwd/environment/jobs and stale input. They do not run native Claude
or kill an ordinary Claude CLI creator. The October 17 real Claude demo above
proves tab closure and public reattachment to its unanswered question in an
experimental Machine home; installed-store acceptance remains open.
The headless OpenCode stand-in Flow proof checks original step/capture identity
after creator death.
An explicit native transition settles the old capture as interrupted, returns 130
from its step and holds the Flow with exit 3, so Task execution does not
automatically retry it.
The existing generic Process outcome remains `failed` for nonzero returns; the
exit code and capture receipt retain the interruption distinction.

Review fixed concrete failures: native transition could trigger Flow replay;
a passive socket could release another viewer's known token; a late attach could
receive exit before the owner drained final output. Capture settlement stays with
the executor, detach requires the same socket's current token, and exit follows
drain. Native-terminal coverage also checks complete final OSC reports and their
absence from Session history. Killing the ordinary interactive creator with its
request pending exposed vt100's one-row wrap panic: unset terminal dimensions now
use 24×80, and collapsed panes retain at least two rows/columns. Reattachment also
signals the terminal's foreground group to repaint even at unchanged dimensions.
The simplification review removed duplicate resume preconditions and obsolete
repair-launch naming/branching; retained output and handoff capabilities now have
one owner each.

October 17 implementation: attachment acknowledgement has a five-second deadline,
cleared only by `Attached`, including observers without a terminal or input token.
The silent-owner fixture accepts a public Session attachment while real execution
continues independently. Timeout and SIGINT exit only the viewer, leave the capture
unsettled and provider claim unchanged, and submit no buffered input. Reattachment
answers the original request. Established control and observe connections remain
usable beyond the deadline. Review retained existing signal cleanup and input gating;
no new cancellation authority or retry path was needed.

The remaining acceptance includes real SSH attach and outage/reconnect,
missing startup acknowledgement, owner crash, twelve retained Sessions,
remote/installed-store acceptance and exact terminal-mode restoration.
Automated local attachment, nested CLI ancestry, slow-viewer isolation and migration
frontier checks ran at gate. Flow OSC acceptance must cover step clearing, linear
progress, idle hand-back and explicit blocking in a real terminal path; the
existing final-frame and parser tests do not establish all of those behaviors.
Configured native provider/Desktop judgment belongs to demo;
fixtures establish transport behavior, not installed provider UI compatibility.

Restoration limits: vt100 covers text grid, 2,000 scrollback rows, alternate screen,
cursor/application keys, bracketed paste and mouse modes. Modern keyboard extensions,
images, hyperlinks, clipboard requests and exact reconstruction of both screen
buffers are not proven. Native processes keep their own drafts. Terminal protocols
outside the library's supported state need evaluation before exact restoration is
claimed. This is remaining acceptance, not a waiver of mode restoration.
Headless observation has a bounded output tail; authoritative final
answers remain in Session capture/history.

Gate checks, October 17 (inherited `LF_*` unset, stdin closed; Rust fixtures use `scripts/test_network.py` except clones, which install their own inherited sandbox):

- `uv run python scripts/test.py --reuse-passing` — initial gate found the architecture-map and Rust regressions above; Desktop built and 600 tests passed headlessly, website 75 passed, Swift boundaries passed; no second full matrix was run after repairs.
- `uv run python scripts/materialize_rust_tests.py --prepare-cli -- cargo nextest run -p loopflow -E '(not binary(=loopflow) or test(flow_tests::) or test(task_flow_launch_tests::) or test(transport::)) and not test(data_clones::)' --build-jobs 4 --test-threads 4 --no-fail-fast` (network runner configured) — 726/739 passed before the final repairs; failing cases were repaired and checked below; canonical draft migration exercised.
- `cargo test -p loopflow --lib flow_tests:: -- --test-threads=4` — 29 passed after Flow crash-fixture repair.
- `cargo test -p loopflow --lib task_flow_launch_tests:: -- --test-threads=4` — 27 passed after admission/placement repair.
- `cargo nextest run -p loopflow --test integration --test session_lifecycle_tests --test transport_tests -E 'binary(=session_lifecycle_tests) or binary(=transport_tests) or test(lf_pr_land_returns_before_later_checks_repair_and_observe_merge) or test(surviving_release_notes_provider_retains_target_checkout_and_context) or test(scheduled_release_tests::)' --build-jobs 4 --test-threads 4 --no-fail-fast` — 44/48 passed; the four remaining cases were checked separately after fixes/diagnostics below.
- `cargo nextest run -p loopflow --test integration --test session_lifecycle_tests -E 'test(public_reconciliation_survives_controller_death_without_republishing) or test(surviving_release_notes_provider_retains_target_checkout_and_context) or test(unavailable_store_starts_no_provider) or test(sigint_records_session_interruption_and_retires_the_orphan)' --build-jobs 4 --test-threads 4 --no-fail-fast` — reconciliation, unavailable-store error and interruption passed; release fixture still needed its captured-event lookup correction.
- `cargo test -p loopflow --test integration surviving_release_notes_provider_retains_target_checkout_and_context -- --nocapture` — passed after selecting the actual executor through its captured event.
- `cargo nextest run -p loopflow --test integration -E 'test(public_reconciliation_survives_controller_death_without_republishing)' --stress-count 5 --build-jobs 4 --test-threads 1` — 5/5 passed in 33.8 seconds; the earlier parallel failure did not reproduce, and its assertion now includes outcome/log diagnostics; no automatic test retry added.
- `cargo nextest run -p loopflow --test transport_tests --build-jobs 4 --test-threads 4` — final 10/10 passed in 14.0 seconds, including EOF and slow observer isolation.
- `cargo test -p loopflow --test integration data_clones:: -- --test-threads=4` — final 4/4 passed, including new conversation, source immutability and source-socket/network denial.
- `cargo test -p loopflow --test integration session_cli_tests::terminal_titles_follow_session_rename_and_reconnect_without_provider_accounts` — passed with the real combined-PTY output fixture.
- `uv run pytest python/tests/test_third_party.py` — 13 passed; notices match the new terminal dependencies.
- `cargo clippy --all-targets -- -D warnings`; `cargo fmt --all -- --check`; `git diff --check` — passed on the repaired code.
- `uv run python scripts/check_architecture.py`; `uv run python scripts/check_migrations.py` — passed; complete owner map, one draft, 76 released migrations unchanged.

All reproduced Rust failures have passing focused follow-ups. The initial aggregate
is retained as failed, not relabeled a clean final-tree matrix; CI owns that fresh
matrix. The single nonreproducing reconciliation failure remains visible above.
No live SSH, installed-store or unsupported terminal-mode acceptance is claimed.
The existing Claude demo evidence and its experimental-home limit are retained in
the architecture reference as well as this plan.

The pre-implementation authority counterexamples and full original handoff remain
in [preserved evidence](evidence/loo-415-before-owner.txt). They explain why process
survival alone was insufficient. Supporting research:
[ownership](session-ownership-research.md), [readiness](session-readiness.md),
[recurring failures](session-continuity-failures.md).
