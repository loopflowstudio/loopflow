# Native skill invocation

```sh
uv run python scripts/benchmarks/skill-invocation/probe.py \
  --claude /path/to/claude --codex /path/to/codex
```

Runs six bounded, live-provider probes in a temporary ordinary directory. Uses
existing provider authentication without reading credentials. No installed skills
or settings are edited. Claude has no model tools or MCP servers; its context
hook reads one fixture file. Codex runs read-only with user config disabled.
These calls consume normal provider usage.

The output records versions, elapsed seconds, marker observations and whether
Claude's replayed native arguments match. A plausible model answer does not prove native
dispatch or argument substitution. Exit zero requires both standalone invocations
and Claude's separate context hook to return their fixture markers; Claude must
also replay the exact original argument. Prefix, suffix and multi-block cases
record counterexamples without assuming future providers retain those limitations.

On 2026-10-07, Claude 2.1.293 recognized standalone stream-json invocation.
Prefixing context or using separate text blocks bypassed native expansion.
Suffixing context put it inside the provider's `<command-args>` receipt. A
`UserPromptSubmit` hook supplied context while preserving the argument. Codex
0.160.1 `exec` returned the `.agents/skills` fixture marker for its `$name`
invocation; this script does not inspect Codex's native expansion receipt.

These are synthetic native fixtures. They establish neither third-party ports,
frontmatter/tool/model fidelity, bundled-file behavior, IDE handoff, Session
continuation, nor Loopflow capture. The one-shot timings are diagnostic samples,
not an lf-versus-slash-command latency comparison. App-server's explicit skill
input is documented but has not been exercised by this script.

## Native continuity

```sh
uv run python scripts/benchmarks/skill-invocation/continuity.py \
  --provider codex --executable /path/to/codex
uv run python scripts/benchmarks/skill-invocation/continuity.py \
  --provider claude --executable /path/to/claude
```

Use the provider binary directly to avoid terminal wrappers. Each run creates
synthetic skills in a temporary ordinary directory and uses native authentication.
Claude disables model tools and MCP; Codex uses read-only sandboxing, disables
MCP/apps/plugins through launch overrides, and asks for no tools. Existing skills,
settings and conversations are not edited. These calls consume provider usage and
leave their new native conversations available for receipt inspection after the
temporary fixtures are removed. Output names those conversations and transcripts.
Each protocol wait is bounded to 55 seconds; cleanup waits five seconds before
killing only the probe's process group.

Codex sends an explicit `skill` item beside the invocation and a separate text
context item through `turn/start`. After restarting app-server, it resumes the
same thread and repeats with a new argument, supplying context again. This checks
history retention and new input delivery, not recall without context resupply.
Checks require returned skill/path and text items, unchanged first-turn history,
and native user-role expansion of
the selected file for each turn. Claude checks native command replay, source
directory and substituted arguments across three processes. Before its second
turn, the probe deletes the hook program and omits hook settings; the original
context attachment must survive. The first answer and reasoning cannot echo the
context marker, preventing answer recall from masquerading as context retention.
The third process explicitly reinstalls the hook. Receipt parsing has offline
tests for wrong sources, altered arguments, assistant echoes and stale turns.
Both scripts share Claude launch settings and receipt parsing; continuity consumes
decoded events directly. Run the offline checks with
`uv run pytest scripts/benchmarks/skill-invocation/test_continuity.py -q`.

On 2026-10-07, Codex 0.160.1 and Claude 2.1.293 passed these transport checks.
**This is not a fidelity pass:** Claude records `hook_additional_context` with
`renderedRole: "system"`. This rendering field does not establish the role sent
to the model. [Claude's glossary](https://code.claude.com/docs/en/glossary#system-reminder)
describes reminders inside user messages or, for some models, system-role messages.
The rendering-only probe did not prove an authority conflict; the request-mapping
probe below now establishes it for the tested binary and model aliases. Cross-session hook
isolation, active-turn steering, GUI handoff, third-party controls and Loopflow's
own capture/dispatch remain unproved. The probe does not use Codex's experimental
`additionalContext` field; its context is a separate user text block.

## Claude request mapping

```sh
uv run python scripts/test_network.py uv run --no-sync python \
  scripts/benchmarks/skill-invocation/request_mapping.py --channel queued
```

Runs the real Claude client against a local fake Messages API with external
egress denied, a disposable home/config and dummy authentication. No live model
or real credentials are used. Output includes the binary version/digest,
requested model, actual request roles, selected source, exact native arguments
and request count. Each observation groups pass/fail assertions under `checks`;
all captured requests contribute to its models, roles and count. The fake response
contains no fixture markers. Run its
offline assertions with `uv run pytest scripts/benchmarks/skill-invocation/ -q`.

`queued` sends a user context message with `shouldQuery: false`, then the exact
slash command. `staged` saves that context and exits without an API request,
then resumes the native Session to invoke the skill. Both then resume again
with a new argument and no context resupply. Success requires one model request
per invocation, user-only context and native source/argument agreement.

On October 7, Claude 2.1.294 passed queued and staged delivery for Sonnet;
queued delivery also passed for Opus and for 2.1.293's Sonnet. `--channel hook`
is the counterexample: 2.1.294 sends the context in the API `system` field for
both model aliases, initially and after resume. It correctly exits 1 because
the user-role requirement fails. This settles client request mapping; it proves
no live-model behavior, Loopflow capture, live-terminal admission, tool policy,
cross-harness translation or startup-cost improvement.

Headless resume now reserves its next capture and claims the driver in one SQLite
transaction after provider-custody checks. Publication retains the new context and
process request; failure releases only that driver and records no provider turn.
CLI regressions for Claude and Codex preserve a held owner's input and history;
store/capture tests cover losing claims, handoff and publication failure. These
prove preservation, not delivery through a held owner. The harness API still needs
structured subsequent-turn input and a current-owner transport.

## Native terminal admission

```sh
uv run python scripts/test_network.py uv run --no-sync python \
  scripts/benchmarks/skill-invocation/request_mapping.py --channel terminal
```

Reproduces two rejected delivery paths using a real Claude terminal, a disposable
home and a fake local API. Only the probe's own PTY and native inbox receive input.
The SessionStart fixture child consumes its own socket capability from the
environment without recording it. External egress is denied; no real credentials
or model are used. Exit zero means the counterexamples reproduced, not that live
skill delivery works. A missing inbox or changed provider behavior fails the probe.

Claude 2.1.294 produced three requests under one native Session
ID: startup `/lf-mapping alpha` expanded the exact source and arguments with
user-role context; PTY injection appended `/lf-mapping beta` to an unfinished
draft and submitted both without expansion; the authenticated inbox delivered
`/lf-mapping gamma` as text without expansion. `/exit` returned zero. The fake
response contains no markers. These are API receipts, not assertions about a
model's answer. Plain PTY injection loses editor fidelity; successful socket
delivery does not prove native command handling.

The tested binary's inbox handler deliberately skips slash parsing. The documented
[background-resume path](https://code.claude.com/docs/en/sessions#resume-a-running-background-session)
also refuses slash-prefixed prompts. An LF terminal over the persistent stream
driver is a concrete alternative, but replaces native interactive controls and
requires a product decision. These findings reject the tested injection paths;
they do not establish that replacing the terminal is unavoidable or prevent
catalog and structured dispatch implementation. Loopflow admission and third-party
fidelity still require proofs through Loopflow's own entry points.


### Explicit Claude plugin selection

Run `request_mapping.py --channel plugin` under the same network wrapper.
It loads the fixture through a private plugin whose skill directory is a symlink
to the unchanged source. On Claude 2.1.294, initial and resumed requests each
expanded exact arguments with user-only context in one model request. The strict
`selected_source` assertion fails: Claude reports the plugin symlink, not the
original directory, as its base directory. Exit 1 preserves that distinction.
This is a candidate for unambiguous startup selection; bundled assets, source
identity through symlinks, retained source contents, and live admission remain
unproved. The probe neither replaces native editing nor establishes lf dispatch.


## Headless LF dispatch and retained Flow sources

```sh
uv run python scripts/test_network.py uv run --no-sync python \
  scripts/benchmarks/skill-invocation/request_mapping.py --lf target/debug/lf
uv run python scripts/test_network.py uv run --no-sync python \
  scripts/benchmarks/skill-invocation/request_mapping.py --lf target/debug/lf --flow
```

October 8's local implementation with Claude 2.1.294 passes both. The direct
command runs in an ordinary folder without `.lf`; the taskless Flow deletes the
selected file after capture, exposing a personal same-name collision before the
child executes. Both send one model request, exact native arguments and user-only
context. The native base directory is a capture-local snapshot whose unchanged
SKILL.md and linked sibling file are checked; manifests retain the original path
and arguments. These prove LF entry dispatch and sibling reachability, not
parent-relative paths, missing bundles, every declaration, live models or admission.

A context-only `shouldQuery: false` message produces its own result. The stream
driver must not declare completion on that acknowledgement. Its focused regression
requires the skill's response before TurnCompleted. Follow-up turns use native
history instead of re-executing the original slash command.

`--channel command-plugin` reproduced a rejected alias-manifest transport:
arguments expand but native skill-directory context does not. `snapshot-plugin`
expands the snapshot directory and arguments across resume; its original-path
assertion intentionally fails. Neither is a claim that paths are interchangeable.

## Codex captured-path counterexample

Prepare the script environment before running with external networking denied:

```sh
uv run --script scripts/benchmarks/skill-invocation/codex_request_mapping.py --help
```

```sh
uv run python scripts/test_network.py uv run --offline --script \
  scripts/benchmarks/skill-invocation/codex_request_mapping.py
```

Uses Codex 0.160.1, private HOME/CODEX_HOME, no credentials and a local fake
Responses API. Each mode ends with one JSON report containing request count and
all checks; catalog paths and native title counts appear where applicable.
Missing requests fail the checks. Exit zero means the counterexample reproduced. Two requests prove
that an explicit captured `skill` path is ignored until registered, then expands
when registered. `skills/list.perCwdExtraUserRoots` is silently ignored. Its
replacement `skills/extraRoots/set` replaces engine-global roots; clearing those
roots removes the registration. This contradicts the documented per-cwd approach.
The [versioned skill-input selector](https://github.com/openai/codex/blob/rust-v0.160.1/codex-rs/skills/src/selection.rs)
requires membership in the loaded catalog; the
[versioned root tests](https://github.com/openai/codex/blob/rust-v0.160.1/codex-rs/app-server/tests/suite/v2/skills_list.rs)
cover replacement. A successful turn alone proves no skill expansion.

The LF prototype reproduced exit zero without expansion and was removed, together
with the `--lf` probe tied to its private snapshot layout (retained at
`c91648d63:scripts/benchmarks/skill-invocation/codex_request_mapping.py`). Native LF
dispatch still needs an entry-point proof once its placement is implemented.
Registering one Session's snapshot must not overwrite roots used by other
conversations in the same engine. No global-root replacement or shared-engine
rejection was added to production.

### Additive Codex catalog selection

```sh
uv run python scripts/test_network.py uv run --offline --script \
  scripts/benchmarks/skill-invocation/codex_request_mapping.py --additive alias
```

October 8, Codex 0.160.1: a symlink in a disposable repository's `.agents/skills`
exposes a captured definition without changing the engine's extra roots. The
probe seeds a separate extra root and verifies it remains listed. An explicit
skill input expands the canonical capture path. Fresh conversations before and
after the mount both expand the original personal `$audit` source when the
snapshot receives a unique `name` alias. Five fake-API requests establish these
separate boundaries; no real model or credentials are used.

`--additive original` is the counterexample and exits 1: keeping the snapshot's
original `name: audit` preserves both catalog entries but makes a fresh plain
`$audit` invocation expand neither. Checking an already-used thread would hide
that failure because its earlier skill expansion remains in history.

The alias is a candidate, not LF dispatch. Production placement, cleanup across
cancellation/handoff, caller checkout preservation, catalog pollution/cost, native
name semantics and complete assets/declarations remain unproved. The probe changes
only its private fixture. It installs no global skills or settings. The official
[App Server reference](https://learn.chatgpt.com/docs/app-server#skills) still
documents the ineffective per-cwd field and separately describes process-wide root
replacement; provider receipts govern these version-specific observations.


## Structured steering and redelivery

```sh
uv run python scripts/test_network.py uv run --offline --script \
  scripts/benchmarks/skill-invocation/codex_request_mapping.py --redelivery
```

October 8, Codex 0.160.1: five fake-API requests distinguish duplicate steering,
single steering and a fresh-turn control. The server holds each initial response
until the client submits the steer, so the input addresses an active turn.
Two identical `turn/steer` messages, including the same JSON-RPC id and expected
turn, both succeed and appear twice in the next model request. A single steer
appears once. Neither expands the explicit skill input. A fresh `turn/start`
with the same skill path and arguments expands it natively.

Exit zero reproduces these counterexamples; it does not certify delivery.
Both acknowledgements are collected as evidence, not literally dropped. This
shows why resending after a lost acknowledgement can duplicate input. It proves
neither LF admission nor cancellation/handoff recovery. No live model, credentials,
native terminal or external side effects participate. A successful steer receipt
cannot stand in for native expansion or application exactly once. The official
[App Server reference](https://learn.chatgpt.com/docs/app-server#steer-an-active-turn)
describes steering and its turn-id acknowledgement; that acknowledgement is not
an idempotency contract.

Current-owner skill delivery needs a native turn boundary and durable correlation
before any retry. Widening LF's text-only `send_current` would retain both faults.
Existing best-effort Task steers are not a Session skill-input queue.

## Native turn recovery and terminal drafts

```sh
uv run python scripts/test_network.py uv run --offline --script \
  scripts/benchmarks/skill-invocation/codex_request_mapping.py --boundary
uv run --with 'websockets>=15,<16' pytest -q \
  scripts/benchmarks/skill-invocation/test_codex_request_mapping.py \
  scripts/benchmarks/skill-invocation/test_request_mapping.py
```

October 8, Codex 0.160.1: the invoking connection completes a baseline turn,
then sends a native skill at the next turn boundary with exact arguments and
separate context. A Unix-socket proxy discards the `turn/start` reply before it
reaches that connection. The caller cancels its pending wait and disconnects
while the fake API holds the model response. A successor connects to the same
engine and reads history without issuing another turn.

The installed schema's `clientUserMessageId` is retained as `clientId` on the
native user message. The probe uses a capture-shaped opaque value here, rather
than inferring identity from prompt text or JSON-RPC ids. History supplies exactly
one matching user message, selected skill path, arguments and context; the earlier
turn remains unchanged. One case completes, and another explicitly interrupts the
recovered turn. Both retain their identities and distinct outcomes after the
fixture engine stops and a new engine reads persisted history. Neither resubmits
input. Missing or duplicate receipts cannot establish successful application.
This establishes receipt correlation, not provider idempotency for repeated ids.

The successful case also attaches Codex's real native terminal to that same thread,
waits for its resume reply, types an unfinished draft, and leaves it in the editor
while the external invocation runs. Only afterward does the fixture press Enter:
native history contains the exact draft on its own turn. It is absent from the
skill request. The terminal exits cleanly. The harness separates typing `/exit`
from Enter so Codex's paste handling does not leave the exit command in the editor.
No replacement terminal or production PTY injection was introduced.

Seven fake-API requests were observed: two baselines, two native skill turns, the
preserved draft and two native title-generation requests. The last two remain in
the report; five conversation requests are not the total cost. Reading a previous
expansion in the draft's history is not another expansion. Regression tests keep
assistant echoes, repeated text, duplicate receipts and historical expansion from
becoming false-positive recovery evidence.

These are provider-level proofs with existing on-disk skills, private homes and
no credentials or live model. They do not establish LF capture admission, active
capture preservation, driver fencing/handoff, captured catalog placement, native
controls in full, arbitrary bundle fidelity, external effects exactly once, or
Claude delivery. LF must connect its retained invocation to this native field
under its existing ownership rules; passive history grants no new authority.
No automatic retries were added. The official
[App Server reference](https://learn.chatgpt.com/docs/app-server) supplies the
thread/read, turn/start and socket interfaces; the generated 0.160.1 schema and
real-client receipts establish the correlation field used here.

## Competing starts at a native boundary

```sh
uv run python scripts/test_network.py uv run --offline --script \
  scripts/benchmarks/skill-invocation/codex_request_mapping.py --boundary-race
```

October 8, Codex 0.160.1: one socket reads a completed baseline; a second
socket starts the same thread before the first sends its skill `turn/start`.
The fake API holds the competing response until the skill request is accepted.
The second start joins the competing turn. Native history retains exactly one
matching `clientId`, selected skill/path, exact arguments and separate context,
but the model request contains no native expansion. A subsequent fresh turn
expands the same skill. Four fake-API requests establish this counterexample;
exit zero means it reproduced, not that admission works.

Idle observation followed by `turn/start` is not atomic boundary admission.
Matching receipt content proves retained bytes, not native expansion or a new
turn. LF's driver fence serializes writes against handoff; it does not currently
reserve a native boundary against competing starts. Delivery needs that boundary
contract before pending-input consumption can rely on it. This probe uses two
protocol clients, not a terminal, LF admission or live models. The earlier
lost-reply/draft probe still establishes its separate sequential case.

## Native queued admission against an attached writer

```sh
uv run python scripts/test_network.py uv run --offline --script \
  scripts/benchmarks/skill-invocation/codex_request_mapping.py --queue-race
```

October 8, Codex 0.160.1: `thread/queue/add` preserves an invocation while an
already-attached client owns the active turn. A proxy drops the enqueue reply;
the caller cancels its waiter and disconnects. A successor recovers the exact
submission using `thread/queue/list`, without resubmission. An explicit
`thread/queue/start` while busy returns an active/pending-turn error and preserves
both the submission and active turn. After that turn completes, the skill expands
on a separate turn with one matching native message identity.

The same case keeps a real native terminal attached with an unfinished draft.
The draft stays out of the skill request and later submits intact. The competing
turn, earlier history and another thread's work survive. Completed invocation
identity and outcome survive an engine restart. Seven fake-API requests include
two title requests; only one fresh skill expansion counts. Exit zero establishes
these provider boundaries, not LF admission or Task acceptance.

These methods come from the installed binary's experimental generated schema:

```sh
codex app-server generate-json-schema --experimental --out /tmp/codex-schema
```

The fetched official App Server reference does not document the queue methods.
The fixture changes neither the protocol nor global skill roots and uses no real
credentials, live model or replacement terminal. It does not prove
repeated-ID idempotency, LF active-capture preservation,
full controls/resources or Claude parity. Loopflow's driver still attributes all
events to one capture and returns on its first completion; integrating queue
consumption must associate each input with its capture and own terminal outcome.
Queue and receipt observations alone grant no LF dispatch authority.

## Headless queue consumption and pending-input recovery

```sh
uv run python scripts/test_network.py uv run --offline --script \
  scripts/benchmarks/skill-invocation/codex_request_mapping.py --queue-headless
uv run python scripts/test_network.py uv run --offline --script \
  scripts/benchmarks/skill-invocation/codex_request_mapping.py --queue-restart
```

October 8, Codex 0.160.1: `--queue-headless` repeats the lost-reply,
cancelled-waiter and connection-handoff case without starting a terminal.
The active turn, sibling and previous history survive; the skill expands on its
own turn without a successful `thread/queue/start`. Four model requests and no
title requests reach the fake API.

`--queue-restart` kills the fixture engine with SIGKILL while the fake API holds
the active turn and two accepted inputs remain queued. A fresh engine reads back
the identical queue bytes before resume. Native `thread/resume` then consumes both
inputs on distinct turns with unchanged content and client IDs, retaining the
interrupted turn's input history. The skill expands once. Three model requests
and no title requests are observed. Nothing is re-enqueued and this case sends no
`thread/queue/start`; it tests pending-input recovery, separately from restarting
after completed history has already been written.

Resume consumes the queue asynchronously. A read can show an empty queue before
its user message appears in turn history; that observation cannot authorize
resubmission. The fixture waits for the exact retained IDs and terminal outcomes.
It does not establish repeated-ID idempotency or external-effect execution once.
These provider-only fixtures do not traverse LF admission, driver transfer,
per-capture event attribution or settlement.

## LF dispatch receipts and reconnect

```sh
uv run python scripts/test_network.py uv run --offline --script \
  tests/e2e/codex_connect.py --codex /path/to/codex --lf target/debug/lf \
  --launch --public-connect --output /tmp/lf-input-receipts
```

The headless LF writer retains exact outgoing input, its capture, native message
ID, Process and provider generation before its first captured `turn/start`.
It consumes the launch's seed capture once; later starts on that writer remain
fenced but have no retained input mapping. This is not pending-input consumption.
`session connect` recovers native user messages across all turn pages without
resubmitting. Missing and
duplicate receipts remain recorded uncertainty; receipt observation grants no
new driver, native-turn origin, completion or external-effect claim.
`content_matches` compares the exact ordered input blocks, disregarding only
empty native text-editor annotations. A single changed receipt is false; missing
or duplicate matches are null. True establishes no skill expansion, as the
competing-start probe demonstrates. Native message IDs cannot be reused by
another capture in the same Session.

A capture can include a retry's different continuation, so its key cannot itself
be the identity of every native message. Each dispatch retains its own
`clientUserMessageId` alongside the capture. A handoff must reuse the retained
mapping; a new continuation has a distinct message ID.

On October 8, the source CLI with Codex 0.160.1 passed exact-text recovery across
two driver handoffs with unchanged active capture, sibling conversation, stale
client rejection and nested Process ancestry. The native terminal client is a
controlled protocol fixture. The storage test `codex_input_dispatch_survives`
retains structured input after an uncertain socket write, refuses repeat dispatch
under both old and new drivers, and keeps empty/duplicate receipt sets without
settling a turn. These checks do not establish new held-owner skill admission,
LF lost-RPC-ack recovery, LF terminal draft preservation or native skill expansion.
The provider-only boundary probe above supplies its own narrower evidence.

## Decisions and remaining work

Jack Heart selected native invocation on the skill's own harness, translated
ports on the other harness, and inlined Loopflow builtins on October 7, 2026.
Jack then selected `--agent` / `-a` for the harness and optional model, and
removed `--ide` after the installed reconnect probe below. The selector rename
and app-launch removal are implemented. The unified catalog now selects skill
sources; Claude headless native delivery is implemented locally, while other
transports, complete ports and admission remain unfinished. Provider-only and LF
entry probes establish the separate boundaries described above.

[LOO-420](https://linear.app/loopflow/issue/LOO-420) owns implementation and
acceptance: one catalog, faithful native dispatch, translated ports, existing
Session admission, unchanged third-party fixtures and matched startup/context
measurements. The working design lives in `scratch/run-any-claude-or-codex.md`
while the Task is in progress.

### Installed app-launch counterexample

On October 7, installed lf 0.13.9 launched Claude and Codex with `--ide` in
separate disposable data directories. Both commands exited zero and recorded a
launcher Process and interactive Session, but no native thread, endpoint,
provider PID or birth identity. Both actual `session connect` commands exited
1 because no connection or confirmed engine exit was recorded. OS acceptance
proved neither visible app execution nor a reconnectable conversation. The
candidate removes this path; it does not repair historical app Sessions.

The installed binary SHA-256 was
`4cf8c9e9a39a916dbe57be9abc3e6eb688ed3ad776ff9243b68537d69592103a`.
The disposable Sessions were `session_787c00c79b9142e38145b3b927fe147c`
(Claude) and `session_8896e66e4c034126ab7ec36aca19b899` (Codex). Native probe
receipts remain in Codex thread `01a11922-4b62-7011-9213-ce33542394a3` and Claude
Session `a162c8e3-4001-43fe-af9b-d6af17181c38`. These are evidence identities,
not control authority over a running provider.

## LF driver settlement and native queue

```sh
uv run python scripts/test_network.py uv run --script tests/e2e/codex_connect.py \
  --codex /path/to/codex --lf target/debug/lf --queued-exit \
  --output .lf/tmp/native-queued-exit
```

Requires one held native turn and a later queued input to survive public
`session connect` driver exit, retaining the endpoint, open Session and pending
bytes, then completing on distinct turns without resubmission. The extra
inspector unsubscribes before exit. This fixture uses the real Codex engine,
fake local Responses and the controlled native-protocol client; no live
credentials or model, replacement terminal or installed store is involved.
A failure remains nonzero. The current engine-close path fails this acceptance.

October 8's removed engine-retention candidate passed queue preservation but
failed the existing `--shared-provider-home` proof: plain Codex could not resume
its saved conversation (`already has an active writer`). Unsubscribe from a
new subscriber returned success without unloading the thread within five seconds;
unsubscribe on the original LF connection also failed plain resume. Keeping
only the passing queue result would conceal a regression. Both proofs constrain
the lifetime repair. The candidate was removed; production admission, per-input
capture attribution and matching completion remain unfinished.
