# OpenCode 2 integration audit — 2026-10-17

```sh
uv run python scripts/test_network.py uv run --no-sync python docs/reviews/opencode-2/loopflow_turns.py target/debug/lf
uv run python scripts/test_network.py uv run --no-sync python docs/reviews/opencode-2/loopflow_turns.py target/debug/lf --routing-and-failure
uv run python scripts/test_network.py uv run --no-sync python docs/reviews/opencode-2/loopflow_turns.py target/debug/lf --resize
```

The candidate now uses OpenCode 2.0.32's authenticated private server, current
Session/context APIs and full-screen native terminal. Jack Heart's revised
priority is ordinary launch, tools, context, batch execution and completed-session
resume. Automatic continuation after a crash is explicitly unsupported: no
synthetic retry input or admission replay is sent. The checkpoint is published as
[PR #1590](https://github.com/loopflowstudio/loopflow/pull/1590); landing
and supported installation remain subsequent Flow steps. No shared schema or
installed provider was changed by this implementation.

## Current implementation evidence

`loopflow_turns.py` runs the actual candidate CLI and installed OpenCode with a
deterministic loopback model in disposable homes, under the network boundary.
Its latest run passed all eight assertions below (11 model requests). This is
native integration evidence, separate from configured-account acceptance.

| Behavior | Current evidence | Remaining gate/demo scope |
|---|---|---|
| Native full-screen launch, tools and resume | Local PTY: bare launch, explicit resumed input, same native identity; resized Unicode draft remains editable and reaches model; configured-account operator: shell result and remembered answer after reconnect | Interruption during rendering; subjective rendering judgment |
| Quit before submission | Empty resumed composer exits without another model request | Initial `--prompt` retains native automatic submission semantics |
| Headless results and Flow | Real headless tool turn, completed resume, one-step Flow and JSON router pass; selected branch reaches model and unselected branch does not | Full native Task workflow acceptance |
| Context, scratch and skills | Real model requests contain initial and refreshed Unicode scratch; generated plugin directory loads; 300 KB Unicode skill reaches model through final marker; current hook fixture covers compaction refresh and child isolation | Native compaction and child-session isolation |
| Model and account selection | Current Session model API; launch fixture checks model; account-routing unit test passes; configured-account native turn succeeds | Agent settings, additional configured accounts, invalid-model/429/5xx cases |
| Native identity and startup errors | API identity precedes terminal launch; focused fresh/resume and failed-start tests pass; native export reader updated | Cross-provider continuation |
| Ownership, steers and receipts | Focused test freezes initiating capture through attachment handoff and independent steer; repeated snapshot does not double usage; operator retains old/new Process attribution across resume | Actual live-provider viewer handoff, native steer and host-queue acceptance |
| Failed/empty completion | Failed idle and empty success rejected; actual model 400 and SIGKILL fail the command; explicit new input completes in the same native conversation while failed/interrupted captures retain their outcomes | Automatic crash continuation remains unsupported |
| Permissions and cleanup | Current permission reply and interrupt APIs; ownership fencing retained; disposable native server cleanup succeeds | Actual Worktree deny enforcement and stale viewer cancellation |
| Output, attention and stream parsing | Current content/tool/form mapping, conformance fixtures and split UTF-8/CRLF parser tests pass | Wider native attention/retry scenarios |

Gate's native probes used **OpenCode 2.0.32** and candidate
**lf 0.14.5+285aadc9d.dirty**, with disposable homes and external network denied.
The ordinary-operation probe passed eight assertions with 11 model requests.
The routing/failure probe proved selected JSON routing, rejected model failure,
and explicit continuation after both model failure and provider SIGKILL. Native
completion receipts join through their frozen initiating-input origin; the killed
capture never acquires the new input's successful receipt. The resize probe used
a real PTY and headless terminal screen parser: the Unicode draft survived a
120×35 → 130×40 resize, remained editable and reached the model intact.

Review found one unmigrated context-hook benchmark still calling OpenCode 1's
removed callbacks. It now exercises current hooks and proves refreshed system
context, intact user messages and child isolation. Two changed Python fixtures
also required formatting. Broader Rust tests exposed a native-handoff stand-in
that never started a private server; it now uses the shared OpenCode 2 fixture.
The startup-recovery fixture also still suppressed a removed log line; it now
exits before publishing the private endpoint. Both retain their original ownership
and recovery assertions. The corrected startup fixture exposed a production
recovery defect: a confirmed-dead private server with stale Process bookkeeping
and no native ID was treated as live headless work. Reopening now settles that
dead attachment through the existing ownership fence before launching the
prepared input; live work still requires native identity before interruption.
Settlement uses the observed attachment and rechecks death, so a replacement
owner cannot be stopped. The transport-resume stand-in now serves the private
endpoint; the session-name fixture keeps its resume proof separate from shared
launch history. Their original behavior assertions remain intact.
These native proofs use a deterministic model, not configured accounts;
the independent configured-account evidence below remains a separate result.

### Gate verification and bounded fixture repairs

`uv run python scripts/test.py --base 616d73b4dc --reuse-passing` ran the affected
plan once: Rust had 2,666 passes, ten failures and 22 skips; website had 75 passes.
All ten Rust failures were repaired and passed focused follow-ups. The original
aggregate result remains failed; no second full-suite or baseline-only run is
claimed. CI owns the final combined-tree result.

Five failures came from Task fixtures registering `/var/...` and
`/private/var/...` as different repositories on macOS. Admission, scoped Task
navigation and initializing-worktree fixtures now consistently register canonical
paths. Assertions and production ownership constraints are unchanged. The focused
materialized nextest selection passed all six tests; it reported one `LEAK` warning
for `delegation_selects_first_owner_but_unknown_started_location_never_moves`.
That leaked-handle observation remains unresolved; no retry or suppression was added.

The architecture scan also mistook generated `.lf/prompts/` history for current
instructions. It now excludes that history alongside existing historical roots,
with regression coverage; authored `.lf/skills` remain checked. The standard
scanner passes on the actual checkout, and all 21 architecture tests pass.

Focused Rust follow-ups passed: all 13 human-session unit tests, all 21 Session
CLI tests, transport's explicit-resume regression and all four data-clone tests.
The lifecycle suite initially passed 25 cases with one stale error assertion and
one ignored case; the repaired assertion and dead-start recovery each passed
focused reruns. Final `cargo fmt --all -- --check`,
`cargo clippy --all-targets --jobs 4 -- -D warnings`, changed-Python Ruff checks
and the JavaScript context-hook check passed. The native commands above passed
their ordinary, routing/failure and resize probes. Paid live tests, remaining
native acceptance in the matrix, and supported installation are not claimed passed.

The independent repository operator used the existing configured account without
reading credentials: native initial answer, shell tool, normal exit, unchanged
`connect --prepare` argv, resumed history and remembered token all passed.
[Filtered operator receipts](operator-native-results.json) retain three completed
turns, four unique usage-message IDs and correct pre/post-resume Process owners.
The operator's misleading success-error finding is repaired: completed receipts
now contain a null error, covered by the ownership/receipt regression test.
This acceptance used a disposable store; the operator's shared-store attempt hit
the preexisting session-transport schema boundary before provider startup.
Supported installation must follow the normal release/materialization path.

Earlier implementation checks passed: OpenCode library tests (23 passed, two paid live tests
ignored), OpenCode launch/lifecycle tests (five passed), shared terminal launch
contract, context steer receipt tests, account routing and failed-start reopen.
`cargo build -p loopflow --bin lf`, `cargo fmt`, and
`cargo clippy --all-targets -- -D warnings` passed. Gate results are recorded above.

Concrete review repairs: remove log-scraped identity and old endpoints; load a
plugin directory with the native package export; keep one launch configuration;
route terminal account overrides as well as server overrides; buffer SSE bytes
before UTF-8 decoding; isolate observer failures in the diagnostic file; never
convert an empty wake or synthetic retry into a successful captured result.

The investigation below is retained historical evidence from **before** this
cutover. Statements that production is unchanged or that recovery blocks the
adapter describe the earlier scope, superseded by the revised ordinary-operation
priority above. Its recovery counterexamples remain valid limitations; no row
claims complete Claude/Codex parity.

## Historical investigation before the adapter cutover

```sh
uv run python scripts/test_network.py uv run --no-sync python docs/reviews/opencode-2/probe.py
uv run python scripts/test_network.py uv run --no-sync python docs/reviews/opencode-2/native_turns.py
uv run python scripts/test_network.py uv run --no-sync python docs/reviews/opencode-2/managed_recovery.py
```

LOO-504 is unfinished. Jack Heart requested autonomous repair, provider parity
verification and delivery. This audit establishes the protocol migration needed
before claiming a repair. Production code, installed tools and production stores
are unchanged. No PR has been published or landed by this audit.

The launch probe uses installed **OpenCode 2.0.32** and **lf 0.14.5**, disposable Git
repositories, isolated Loopflow/provider homes, an actual PTY and a private
native server. It supplies an ephemeral local transport credential only in
memory. It reads no account credentials and executes no model turns. The separate
`native_turns.py` probe executes native model/tool turns against a deterministic
loopback model with external network denied; it does not run the Loopflow adapter.
The probes share disposable-home and server cleanup in `probe_support.py`;
context managers release the model and stop the provider on failure as well as
success. Forced cleanup fails the probe. `managed_recovery.py` separately injects SIGKILL
as a deliberate crash scenario. Native turn checks use the provider's wait
barrier before reading outcomes; settlement alone proves neither completion nor
recovery. Source inspection used Loopflow base `28dc1416712c59203ac89d0d4128eb0e7f79b085` and
[OpenCode tag v2.0.32, commit 843a6c514e606b436a7fe713422ace0a93ad3d61](https://github.com/anomalyco/opencode/tree/843a6c514e606b436a7fe713422ace0a93ad3d61).
[Probe results](probe-results.json) retain the observed statuses; temporary paths
in them name fixtures already removed.

## Confirmed failures and contrary evidence

- Installed `lf -a opencode` exits 1 in the PTY and reports missing resumable
  identity. Its private log retains the real cause: uppercase `INFO` is rejected.
  The flag diagnostic does **not** overlay the terminal. LOO-501's separation
  holds for this failure; changing the flag alone does not establish launch.
- The root OpenCode 2 command rejects `--model`. `run` and `mini` have model flags;
  replacing the full-screen interface with `mini` would be a product change.
- The old `opencode export SESSION` command fails. Current export lives under
  `opencode session export`. Its messages use the current native schema.
- With valid transport authentication, old `POST /session` and
  `/session/SESSION/prompt_async` return 405. `GET /event` returns HTTP 200 with
  non-JSON web content, not the expected event stream. The old readiness test
  accepts any HTTP response from `/`, including this web page.
- `/api/info` works with authentication and returns 401 without it. Native
  `/api/session` creates a session in a `data` envelope; reading that identity
  succeeds. This proves native create/read, **not** Loopflow resume.
- `/api/session/SESSION/prompt` admits 240,000 UTF-8 bytes exactly with
  `resume: false`. No model or tool runs. This proves native input admission,
  **not** Loopflow prompt transport or model consumption.
- **Contrary evidence:** the provider translates legacy `permission` and `plugin`
  configuration keys. The probe's config documents retain the explicit
  `external_directory` deny in current ruleset form. An initial probe decoder
  incorrectly assumed a `data` envelope for this endpoint; the saved result now
  retains its actual document array. Config translation does not prove old plugin
  callback execution or tool enforcement.

## Architectural counterexample

Loopflow's `opencode_history::native_receipts` selects user-request ownership
through assistant `parentID`, then closes each request from its last completed
assistant. OpenCode 2's
[native message schema](https://github.com/anomalyco/opencode/blob/843a6c514e606b436a7fe713422ace0a93ad3d61/packages/schema/src/session-message.ts)
has no assistant parent request. Its durable idle marker covers the execution
including prompts steered while busy. Private-server shutdown leaves the execution
unfinished; restart alone does not resume it in the probes below.
The [native event schema](https://github.com/anomalyco/opencode/blob/843a6c514e606b436a7fe713422ace0a93ad3d61/packages/schema/src/session-event.ts)
instead exposes inbox admission/delivery, model steps and execution results.

October 17 native experiments now establish an additional counterexample to the
proposed recovery design. [Native turn results](native-turn-results.json) retain
provider-assigned IDs, ordered transcripts, token receipts and outcomes; only
disposable checkout paths are removed. The probe uses no invented input IDs: it
replays IDs returned by native admission. Its assertions reproduce the observed
behavior; a passing probe is **not** repair acceptance.

Fixture synchronization matters: a later run stopped after model-request arrival
but before the second assistant was recorded. Readmission still produced empty
success, but there was no aborted assistant row to assert against. The probes
now emit an initial response fragment and wait for the second assistant in live
context before shutdown. Export cannot supply this barrier: the provider's
[transfer implementation](https://github.com/anomalyco/opencode/blob/843a6c514e606b436a7fe713422ace0a93ad3d61/packages/core/src/session/transfer.ts)
filters out unsettled messages. Saved results below retain the earlier observations;
the synchronization change preserves their terminal-outcome assertions.

| Native scenario | Model requests | Observed boundary |
|---|---:|---|
| Pending admission, then mid-step steer | 2 | Admission alone has no messages; native shell runs once; steer reaches the next request; one successful idle |
| Steer plus queued input | 3 | Queue reaches the third request, after the steered response; still only one successful idle |
| Model HTTP 400 after native shell | 2 | Failed assistant and failed idle; the completed tool's usage remains |
| Private-server shutdown during second request | 2 | Aborted assistant, preserved tool and usage; no idle after restart |
| Replay original delivered admission after restart | 2 | **Successful idle without another model request; last assistant remains aborted** |
| New synthetic continuation after restart | 3 | A new model request and successful idle; one tool execution in this deterministic experiment |
| Interrupt with resume through three private restarts | 2 | Each response is `interrupted: false`; no idle, transcript change or additional usage |
| Original-admission replay through three private restarts | 2 | Three distinct successful idle markers; no new model request, transcript content or usage |

The production native log endpoint returns only `log.synced`, even after native
execution. The [bus implementation](https://github.com/anomalyco/opencode/blob/843a6c514e606b436a7fe713422ace0a93ad3d61/packages/core/src/bus.ts)
defaults event payload persistence to false; native log tests explicitly enable
it. Sequence watermarks survive, but the replayable event payloads assumed by the
draft adapter do not. Exported message snapshots retain ordered user/assistant
messages and idle markers, and repeated completed snapshots are identical.

A simple transcript reducer also fails the readmission case: selecting the first
user since the last idle and completing it at the next successful idle would mark
unfinished work completed by replaying admission alone. The native
[prompt operation](https://github.com/anomalyco/opencode/blob/843a6c514e606b436a7fe713422ace0a93ad3d61/packages/core/src/session/session.ts)
reconciles the existing input and wakes execution. The
[coordinator](https://github.com/anomalyco/opencode/blob/843a6c514e606b436a7fe713422ace0a93ad3d61/packages/core/src/session/run-coordinator.ts)
starts a non-forced drain for that wake; without pending input it succeeds without
calling a model. Native force-resume is a different operation. The
[managed restart sweep](https://github.com/anomalyco/opencode/blob/843a6c514e606b436a7fe713422ace0a93ad3d61/packages/core/src/session/execution/restart.ts)
is enabled for managed service startup, not private stdio servers.

The synthetic experiment is contrary evidence to a claim that continuation is
impossible: adding new input does continue the native conversation. It is not
transparent native resume, proof of arbitrary side-effect deduplication, or an
accepted change to Loopflow's result/steer contract. Using a shared service would
also require solving launch environment and ownership rather than silently
adopting a previous process.

The follow-up cases can be run independently without repeating the six original
experiments:

```sh
uv run python scripts/test_network.py uv run --no-sync python docs/reviews/opencode-2/native_turns.py --scenario interrupt_resume --scenario repeated_readmission
```

Both cases passed against installed 2.0.32. They use the public interrupt endpoint
with `resume=true` and the native wait endpoint as a settlement barrier. Across
three restarts they assert unchanged non-idle messages and usage, provider-issued
input identity, the original aborted assistant, and exactly two model requests
(the completed tool step and the interrupted step). The repeated-admission case
also asserts three successful idle markers. Passing reproduces the defect; it
does not prove Loopflow completion rejection or receipt idempotence.

The apparent public recovery alternatives do not supply force-resume:

- [Execution interrupt](https://github.com/anomalyco/opencode/blob/843a6c514e606b436a7fe713422ace0a93ad3d61/packages/core/src/session/execution.ts)
  with resume only wakes pending promotable input; delivered input has none.
- [Promise plugin Session](https://github.com/anomalyco/opencode/blob/843a6c514e606b436a7fe713422ace0a93ad3d61/packages/plugin/src/promise/session.ts)
  and [Effect plugin Session](https://github.com/anomalyco/opencode/blob/843a6c514e606b436a7fe713422ace0a93ad3d61/packages/plugin/src/effect/session.ts)
  omit the internal resume operation.
- Public skill activation calls internal force-resume only after publishing skill
  content. Treating it as recovery would add input and change the contract.
- [Server startup](https://github.com/anomalyco/opencode/blob/843a6c514e606b436a7fe713422ace0a93ad3d61/packages/server/src/process.ts)
  installs restart continuity only with managed lifecycle. The CLI disallows
  combining `--service` and `--stdio`.

**Unmet private-server capability:** the inspected installed HTTP/plugin surface has
no standalone operation that force-resumes delivered interrupted work under the
private launch owner without adding or replaying input. An upstream capability
or an explicitly revised recovery design is needed before the dependent adapter
cutover. No installed provider patch, shared service or synthetic continuation
was adopted. This is a technical dependency, not a human approval or auth gap.

**Remaining prerequisite:** that supported recovery operation and durable join that
reject empty successful wakes and preserve the frozen initiating capture through
attachment handoff. No Loopflow handoff, immutable ownership or receipt
idempotence test was completed by these native probes. The attempted
transcript-only adapter edits were withdrawn at this counterexample; production
code remains unchanged. The isolated managed experiment below narrows this gap
to a concrete recovery-semantics decision or upstream capability.

## Isolated managed recovery

`managed_recovery.py` directly owns `serve --service --hostname 127.0.0.1 --port 0`
inside disposable HOME/XDG paths. It uses no shared service, installed-provider
patch, account credentials or admission replay. It waits for each process to exit
before starting its successor. [Managed recovery results](managed-recovery-results.json)
retain native IDs, transcripts and the actual recovered model request for both
SIGTERM and SIGKILL. External network remains denied.

Both cases pass on installed OpenCode 2.0.32:

- Recovery retains the native Session ID and sole original user admission.
- Four model requests occur: original tool, interrupted response, recovered
  verification tool, final response. The completed tool's file append occurs once;
  this controlled model does not prove arbitrary side-effect deduplication.
- Shells before and after restart see the same launch environment marker and
  repository cwd, with neither server-password environment variable present.
- One successful idle follows actual recovered model work. Repeated reads and
  another restart after completion leave the native snapshot and request count
  unchanged. These are native facts, not Loopflow receipt/completion guarantees.
- **Each recovery inserts a synthetic restart notice and includes its continuation
  instruction as a user-role message in the recovered model request.**

This last behavior prevents adoption under the supplied no-synthetic-continuation
constraint. The provider's
[restart sweep](https://github.com/anomalyco/opencode/blob/843a6c514e606b436a7fe713422ace0a93ad3d61/packages/core/src/session/execution/restart.ts)
always publishes that notice before resuming a suspended claim. The
[message updater](https://github.com/anomalyco/opencode/blob/843a6c514e606b436a7fe713422ace0a93ad3d61/packages/core/src/session/message-updater.ts)
persists it, and the
[model-message conversion](https://github.com/anomalyco/opencode/blob/843a6c514e606b436a7fe713422ace0a93ad3d61/packages/core/src/session/runner/to-llm-message.ts)
sends synthetic content with role `user`. No inspected CLI/config switch disables
it. Source also documents at-least-once recovery and a default ten-attempt budget;
the probe does not exercise budget exhaustion or arbitrary tool crash windows.

The exact dependency is either an upstream supported operation exposing internal
`SessionExecution.resume` without content injection, with its suspended-claim
ownership contract, or explicit acceptance of provider-authored restart guidance
and at-least-once recovery. The latter must preserve the distinction between
recovery notices, initiating results and independent steer receipts. Native
ownership and a supported CLI alone do not settle that semantic decision.

Managed mode also changes process cwd to home and persists credentials in config
and service registration, as shown by the
[CLI server process](https://github.com/anomalyco/opencode/blob/843a6c514e606b436a7fe713422ace0a93ad3d61/packages/cli/src/server-process.ts).
The experiment supplies the repository location explicitly, writes generated
fixture credentials at 0600, and disposes of them with the fixture. It proves
neither production account/config access under isolated paths nor a production
credential-storage/AgentProcess cleanup design.

No Loopflow capture-owner implementation was adopted: frozen capture/Process/Work
attribution across handoff, independent steer receipts, separate host-queued
results, failed/empty-result rejection and idempotent usage/completion remain
required. Adapter replacement and its focused build/tests cannot advance until
the semantic dependency is resolved. Gate/CI, publication, landing and installed
verification retain their later owners.

## Historical coverage matrix (before implementation)

“Existing coverage” means inspected test/code coverage, not a fresh passing run.
Claude/Codex columns describe Loopflow's implemented contracts; their installed
providers were not run in this audit. No row claims full provider parity.

| Behavior | Claude/Codex baseline | OpenCode evidence and result | Remaining limitation |
|---|---|---|---|
| Fresh interactive launch | Native Claude terminal; Codex owned app-server terminal bridge | **Native failure reproduced:** real lf PTY, uppercase log-level error | Fix full-screen launch, editable prompt, quit-before-submit and native confirmation |
| Explicit model/agent | Provider-specific launch configuration | **Native failure:** root `--model` rejected; source root has no agent flag | Carry model through current config/session API; verify native agent settings |
| Batch and managed execution | Stream-json/app-server own input and completion | **Native route failures:** old session/prompt APIs rejected | Replace connection, submission, lifecycle and output together |
| Task/Flow execution and retry | Shared agent runner; native evidence selects captured result | `session_lifecycle_tests.rs` uses an OpenCode 1 server fixture | Preserve failed-turn rejection, late results, retry identity and JSON routing on v2 |
| Prompt transport, Unicode and size | Claude/Codex large first-turn tests in `context_launch_tests.rs` | Native API admits 240 KB exactly; Loopflow terminal still uses bounded argv | Prove actual model consumption and terminal size behavior |
| Tools and permissions | Provider approval/sandbox controls | Legacy deny translation confirmed; deterministic native shell returns `native-tool-proof` exactly once per scenario | Loopflow tool routing, explicit deny and Worktree boundary enforcement remain untested |
| Instructions, skills, scratch | Claude hooks/Codex hooks and retained delivery context | Current plugin uses removed `experimental.chat.*` hooks; local JS test invokes these directly | Replace with current `{id, setup}`/Session hooks; verify real plugin loading |
| Refresh and compaction | Shared context producer with provider callbacks | Old plugin-only benchmark tests synthetic message mutation | Native compact/refresh/retry and child-session isolation untested |
| Native identity and resume | Claude supplied ID/Codex protocol ID; saved native history | Native API create/read works; Loopflow terminal scrapes old startup log | Fresh/resumed PTY and failed-start identity unproved |
| Reconnect/cross-provider continuation | Existing Session connect and conversation-log paths | Old export command fails; v2 transcript uses `type`/`content`, not `info`/`parts` | Move log consumer and verify conversation survives provider switch |
| Steering | Shared send-current contract and provider result ownership | Native mid-step steer reaches the next model request; native queue reaches a later request but shares one idle marker | Resolve execution-vs-request semantics before result attribution |
| Cancellation and cleanup | Attachment-fenced AgentProcess lifecycle | Public interrupt/resume fails; readmission emits empty success. Isolated managed SIGTERM/SIGKILL recovery works by adding model-visible synthetic guidance | Requires acceptance of those recovery semantics or upstream content-free resume; Loopflow shutdown/resume and stale-owner rejection remain unproved |
| Output, errors and SSE | Provider trace conformance tests | Old `properties`/part events differ from v2 `data`/step events | Replace mappings and replay fixtures; preserve actionable failures |
| Unicode stream boundaries | Parser must preserve text across arbitrary chunks | Static defect: `String::from_utf8_lossy(chunk)` corrupts split multibyte text | Byte-buffer framing regression, including split CRLF and multiline data |
| Usage attribution | Native receipts, missingness and immutable capture origins | Native snapshots retain distinct step IDs/usage; snapshot totals equal summed measured steps across eight scenarios | Preserve historical receipt reads; do not infer missing counters as zero |
| Status and attention | Rust attention reducer; explicit native wait/result evidence | Old question/status event fixtures only | Current forms, permissions, retry, idle and disconnect evidence untested |
| Terminal rendering/resize | Native UI owns terminal; retained diagnostics | PTY proves startup diagnostics separation only; LOO-501 fixture keeps raw bytes/0600 log | Successful editable prompt, resize, retries and resume on OpenCode 2 untested |
| Auth/model/network failures | Provider-specific errors and account routing | Private server returns 401 without auth; deterministic model 400 becomes failed native execution | Configured Zen/Kimi, invalid model, 429/5xx, network recovery untested |
| Account discovery | Claude/Codex managed account selection | Loopflow reads legacy `auth.json`; provider source retains `OPENCODE_API_KEY` support | Verify supported native credential discovery without reading/exporting secrets |

## Tests and consumer inventory

Production owners inspected: `agent/session.rs`, `lf/commands/util.rs`,
`harness/opencode.rs`, `opencode_history.rs`, `opencode_mapping.rs`,
`harness/attention.rs`, `harness/context.rs`, `context_opencode.js`,
`session_record.rs`, `ops/human_session/conversation_log.rs`,
`prompt/process.rs`, and OpenCode paths in `provider_auth/mod.rs`.

Existing coverage to migrate includes `session_launch_contract.rs` (including
LOO-501 byte-exact diagnostics), `session_lifecycle_tests.rs`,
`tests/support/opencode_server.py`, `harness/conformance_tests.rs`,
`harness/testdata/`, and the OpenCode context benchmark. The diagnostic failure
test currently asserts the misleading missing-session error. The server fixture
implements the retired endpoints, so its success cannot prove installed v2
behavior. The ignored `live_*` harness tests use those same retired endpoints;
running paid model tests before the transport cutover would add no useful proof.

Review findings: parent-request grouping cannot read v2, production log
watermarks do not imply replayable payloads, and a successful idle after an empty
wake cannot prove interrupted work completed. These findings stopped the
transcript-only replacement before it became a production completion authority.
The managed experiment adds a concrete review finding: supported native recovery
inserts model-visible instructions, so isolation alone cannot satisfy the selected
semantics. The revised draft retains one adapter and one deletion cut once that
dependency and immutable capture attribution are resolved.

All repair, gate/CI, configured-account acceptance, publication, landing and
released-path verification remain outstanding under **LOO-504**. No duplicate
gap Tasks were created. **LOO-454** still owns context-consumption accounting;
**LOO-502** still owns upgrade continuity. Official current references:
[CLI commands](https://dev.opencode.ai/v2/docs/cli/commands/),
[HTTP API](https://dev.opencode.ai/v2/docs/api/), and
[plugin Session hooks](https://github.com/anomalyco/opencode/blob/843a6c514e606b436a7fe713422ace0a93ad3d61/packages/plugin/src/promise/session.ts).
