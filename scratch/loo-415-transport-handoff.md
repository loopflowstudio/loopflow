# LOO-415 — live Session transport

## Scope and decisions

Reconciled October 17, 2026 against the working implementation. Owner topology
and native UI requirements are accepted; implementation and restoration coverage
remain under review. The supplied attachment-deadline feedback is addressed by
the implementation and eight recorded transport proofs below. Local `origin/main`
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
The accepted direction supersedes its multiplexer exemption: every execution has
an independent owner. Remote commands use `lf --machine LABEL ...`. LOO-416 owns
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

Focused proofs use the built CLI and disposable Machine homes. Codex's stand-in
requires an answer on the original WebSocket request: creator exit and zero viewers
leave native PID/thread/turn and provider attachment unchanged; replacement answers
without capture settlement or input replay. The Claude-shaped shell stand-in and
shell fixture cover launch-then-attach,
retained shell cwd/environment/jobs and stale input. They do not run native Claude
or kill an ordinary Claude CLI creator. Independent native Claude experiments
prove an unanswered question without viewers, but not lf creator death or attach.
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

Gate still owns the full affected suites, local/SSH attach and outage/reconnect,
missing startup acknowledgement, owner crash, slow-viewer/load limits, migration
frontier and remote installed acceptance. Ordinary Claude creator teardown,
nested CLI launch, twelve retained Sessions and exact terminal-mode restoration
still need their own proofs. Flow OSC acceptance must cover step clearing, linear
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

Checks: reused October 17 `cargo test -p loopflow --test transport_tests -- --test-threads=1` (8) and `cargo test -p loopflow --lib preferred_name_resume_uses_config_when_forwarded_name_is_empty` (1), inherited `LF_*` unset and stdin closed; `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` passed. Reconciliation: `git diff --check` and `lf context` passed. Prior release repair, terminal unit, documented commands and third-party results remain applicable; gate owns broader acceptance.

The pre-implementation authority counterexamples and full original handoff remain
in [preserved evidence](evidence/loo-415-before-owner.txt). They explain why process
survival alone was insufficient. Supporting research:
[ownership](session-ownership-research.md), [readiness](session-readiness.md),
[recurring failures](session-continuity-failures.md).
