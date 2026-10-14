# Reliable provider launches

Jack Heart requested command robustness across terminal integrations rather than
relying on a cmux-specific patch. The incident evidence is in
`cmux-session-reconnect.md`. Jack approved Codex implementation on 2026-10-13;
that change is complete locally, with release delivery remaining. On Oct 14 Jack
approved OpenCode HTTP prompt delivery, investigation of Claude first-message
delivery, and visible startup feedback. Claude background/attach reconnect is
excluded: Jack reported that approach had already been tried unsuccessfully.
Preserve its existing stop-and-resume behavior.

## Remaining improvements

- OpenCode: preserve plugin instructions and the exact first request while moving
  the request to HTTP. Preserve native approvals, ownership, retry and history.
- Verify release readiness and the installed path. No wrapper changes are needed.

Jack reported another 10+ second `lf resume` wait on Oct 14. Its launcher started
at 1791988065 and provider at 1791988066; the installed launch still used 10,862
bytes of Codex argv. These timestamps bound admission to roughly one second,
but do not timestamp execution past the wrapper or terminal readiness. The
protocol fix is not installed. This is consistent with the earlier wrapper
finding, without establishing the exact cause or duration of this restart.

## Direction supported by the probe

Keep provider command lines small: executable, endpoint, cwd, model and short
flags. Deliver instructions, hooks and first-turn content through the provider's
structured protocol or a supported file input. Preserve the content and its
instruction level, native history, user configuration and account selection.

The installed Codex 0.162.1 accepted 1,100,000 instruction bytes in
`thread/start.developerInstructions` through the **unmodified** installed cmux
wrapper. The command line was 94 bytes. Initialization took 0.061 seconds and
the thread response arrived at 0.070 seconds. The probe used a temporary HOME
and CODEX_HOME, a no-op cmux executable and no account credentials. It created
an ephemeral thread and never sent `turn/start`. This proves startup transport,
not a model response, hook delivery or native terminal recovery.

An initial probe omitted creation of CODEX_HOME and exited before initialization;
the corrected probe created that directory. Its cleanup also checks child exit
before signalling. No conclusion about the first failure's exact cause was
established before the temporary diagnostics disappeared.

## Implementation boundary

Codex already has the relevant components:

- `harness/codex.rs`: launches app-server with short arguments and supplies
  developer instructions, hooks, tool environment and input through JSON-RPC.
- `ops/human_session.rs::connect_live_codex`: attaches the native terminal to a
  running thread through an attachment-fenced relay.
- `lf/commands/util.rs` and `agent/mod.rs`: still launch native Codex directly
  with large arguments. Replace that duplicate transport with the existing
  provider startup and native connection path.

Do not make a third launcher, change CODEX_HOME to impersonate another account,
overwrite user profiles, replace Codex's base instructions, or encode whitespace
to accommodate one wrapper parser. The short-message request-file alternative
was raised with Jack but has no recorded answer; it is not required for the
Codex protocol approach.

The native terminal creates or resumes the provider thread on its own connection.
The relay supplies the prepared configuration and submits the saved first input
there, after subscription. Approval events remain on the native connection.
A reconnect with recorded turns never resubmits the original input. Preserve managed-account routing and native CLI options.
Do not silently turn an interactive launch into an unattended headless turn.

Claude already supports an additive system-prompt file; OpenCode's context
plugin already uses a file. Their first-turn arguments still need a supported
transport decision. Do not claim Codex's protocol solves other providers.

## Failed starts must remain reconnectable

The foreground opener currently drops its child after a 30-second readiness
timeout. That can kill the recorder before its provider exits. Define one owner
for startup cancellation, provider termination/reaping and settlement. Report
the failed stage without printing prompts or environment values. Preserve the
input and native history so a retry uses the same conversation.

The merged dead-native-provider observation fix (#1542) is necessary for
unexpected exits and needs release delivery. It does not prevent a slow wrapper
or establish native history. Never turn missing exit evidence into fabricated
success, and never start another provider while the previous one is alive.

## Verification and remaining release work

The actual installed Codex 0.162.1 launched through the unmodified cmux wrapper
in an isolated Loopflow Machine and a `forkpty` terminal. It answered the initial
request in 5.96 seconds, then resumed the same conversation and answered a typed
follow-up in 8.57 seconds. Both exited successfully. The Machine retained one
Session, two started/completed turns, two outputs and no live provider records.
Context hooks ran successfully. The fake cmux endpoint deliberately disabled
pane/title side effects; opening an actual cmux pane was not part of this probe.

Focused CLI fixtures reject any provider argument over 1 KiB while preserving a
440,000-byte skill body with Unicode, quotes, shell syntax and newlines, exact
additive developer instructions, hook configuration and native history. The
terminal declines an initial approval; the bootstrap connection never answers it.
Failure checks cover early provider exit, a stalled wrapper, a terminal that
never connects, and exit after the first turn starts. Reconnect preserves the
captured request and does not replay a recorded first turn. Earlier dead-provider
counterexamples remain covered, with and without published native history.

Release delivery is still needed for this transport change and the already merged
#1542 stale-exit repair. Claude stdin delivery is recorded below; OpenCode remains pending. The
original perceived `lf incident -i -c` hang was not reproduced and remains unattributed.

## Delete — do not maintain

- Claude initial-message argv and its oversized-argument rejection fixture:
  replace with exact stdin delivery and a large-message CLI regression.
- `agent::build_codex_command` and its argv-only tests: use the shared native
  protocol launch, retaining permission and model tests on protocol parameters.
- `harness::context::codex_args`, its inline TOML encoder and the native
  SessionStart identity hook: protocol replies now establish the native identity.
- Native Codex prompt and developer-instruction argv in launch and resume.
  Preserve exact captured input and additive developer instructions.

## Implementation decisions and review

Codex cannot resume a new thread before its first user message materializes
native history. An actual provider probe contradicted the first implementation's
bootstrap-then-resume approach. Bootstrap now only starts and configures the
server; the native client creates the thread. The relay binds its returned ID,
then publishes the native-history receipt when the first turn starts. A shared
pending input prevents duplicate submission by connections in the same launch.

The installed CLI also rejects `--no-daemon` and `--add-dir` with `--remote`.
Remote argv now carries the endpoint and optional native ID. Workspace roots
travel through server configuration, retaining configured roots. The provider
argv measured 125 bytes in the real probe. Claude/OpenCode retain their existing
argument cap. Native and headless callers reuse the existing server owner,
account routing, lifeline and attachment fencing.

Review found that passive resume replies must never re-record an attachment:
a stale terminal must continue displaying while its writes are refused. Only
owned startup binds the returned native ID. The ownership regression also exposed
an existing history-origin lookup that closed the socket on stale input before
RPC rejection. It now rejects that input while preserving display. Failed starts retain the captured
request, rather than substituting the Session's optional summary label. The
foreground readiness timeout terminates and reaps its launcher explicitly.

The scoped native worker keeps terminal I/O on the invoking thread. Startup is
bounded; failures report the provider or terminal stage. No schema changes,
wrapper modifications, account impersonation, or new durable delivery flags.

Checks: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, focused Codex/context-launch/native-lifecycle/stale-exit tests, actual-provider retained-client ownership test and unmodified-wrapper native launch/reconnect passed; release installation and a real cmux pane remain delivery/demo work.

## Oct 14 transport probes

Claude 2.1.299 consumed the exact 440,000-byte request from stdin and accepted a
typed follow-up through the controlling terminal. The native terminal used an
isolated home and a local stand-in Anthropic API, with no external model request.
Temporary evidence: `/tmp/lf-claude-input-probe.py`; output and captured requests
under `lf-claude-input-skgpq3t8` in the system temporary directory. The first
probe's trust configuration used a symlinked path; using the canonical directory
resolved that fixture error. Title generation is a separate provider request;
the successful check distinguishes it from the conversation's initial message.

Claude now uses an anonymous temporary file as stdin, avoiding both wrapper
parsing and pipe-buffer blocking. Its system instructions, native arguments,
keyboard handling and stop-and-resume path remain intact. The new indicator is
terminal-only, clears on failure, and joins its writer before provider rendering.

OpenCode 1.18.33 also reads stdin, but the real terminal probe accepted the exact
message without accepting typed follow-up. Do not switch OpenCode to stdin on
that evidence. Its native HTTP endpoint accepted early session selection without
the UI navigating; a later selection did navigate. Server health alone does not
prove native UI subscription. The server-and-attach approach remains unresolved;
do not replace it with timed sleeps or report these probes as recovery.

Checks: Oct 14 `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all`,
17 native launch unit tests, 10 context-launch integration tests and a real PTY
indicator probe passed. The PTY probe observed stage changes, clearing before
native output and no later writes; redirected output contained no indicator.
Review corrected a remaining worktree test that still expected Claude argv input.
No OpenCode transport edit is included in this checkpoint.

## Actual cmux disconnect regression

The real cmux pane exposed a client-side broken pipe on normal Ctrl-D. The relay
treated abrupt terminal closure as a provider failure. Client-side close/reset
now ends the relay normally; provider errors and nonzero terminal exits still
fail. A real socket regression covers both WebSocket close and abrupt closure.

The first timing probe incorrectly matched text left by the previous terminal,
then sent Ctrl-D before resume subscribed. Its 0.97-second readiness claim is
invalid. That exposed another defect: saved initial input remained marked pending
until the terminal's resume reply, even with existing native turns. Native startup
now reads history before opening the terminal, using the same recovery reader as
live attachment. Tests preserve fresh-start retry and prove early resume exit
does not replay the original input or mask a nonzero terminal failure.

Actual Codex 0.162.1, through the unmodified cmux wrapper in a real cmux pane,
resumed native thread `01a13ae2-5f99-7943-ae9f-99fe8457a2a6`, returned the newly
typed `CMUX_VERIFIED_RESUME_3`, and exited successfully on Ctrl-D. A fresh launch
also returned `CMUX_LAUNCH_OK` and exited successfully, leaving zero live providers.
Evidence: private Machine and terminal scripts in
`/private/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/lf-cmux-launch-hagz2wwe`.
The original timing record is retained with an explicit invalidation. This is
branch-build evidence, not installed-release recovery.

Checks: Oct 14 `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
both Codex context-launch regressions and the relay close regression passed;
real cmux resume/reply/clean exit passed. Installed release verification remains.
