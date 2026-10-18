# Fresh Codex launch recovery — October 18, 2026

Jack Heart reported two fresh `lf demo` launches failing in native Codex 0.163.0
on installed lf 0.14.5. Their UIs lost transport while native conversations did
work. Jack Heart authorized autonomous implementation, validation and delivery.

## Accepted outcome and constraints

Fresh native skill launches display actual work. Reconnect retains the same
thread, draft and history without duplicate input, tools or conversations.
Failures distinguish not submitted, dispatched with known identity, and genuinely
uncertain creation/submission; retain actionable diagnostics. Preserve native
settings/tools, approvals, attachment fencing, account identity, existing Sessions
and LOO-497's checkout. No provider changes or schema migration.

The Task directive is accepted; the repair remains a candidate until the original
failure is explained. Installed success and a version upgrade are not closure.

## Design — implemented candidate, reconciled October 18, 2026

The Session remains the identity/history owner. Reconnect reads its saved native
identity; a fresh renderer's first `thread/start` resumes it. Both response-first
and notification-first startup can save identity. A vanished renderer cannot
cancel an outstanding creation or first-input reply; draining has a fixed deadline.
A client-task failure retains the launch's listener and writes private,
best-effort `codex-relay.log` diagnostics without terminal output.

Initial prompts already live in the immutable captured AgentProcessRequest.
Submission receipts now live alongside them in existing Session observations,
scoped to the captured input. Dispatch uncertainty is written under the same
attachment fence immediately before transport dispatch; a refused stale writer
records no dispatch. Explicit rejection is recorded before pending text/parameters
become available to another connection. A later native launch recovers the saved
prompt only when not dispatched or explicitly rejected. An unacknowledged dispatch
is never replayed automatically. Native turn history can resolve its acceptance.
Creation dispatched without a received identity remains uncertain and cannot
silently create a second conversation. Resume uses the saved Session identity even
when creation never reached the first-turn provider-session receipt.

### Delete — do not maintain

No predecessor targets remain. Keep one Session identity and captured-input
receipt path; do not restore connection-local identity selection, listener exit
on a client error, or consumed text as proof of acceptance. No parallel history,
draft file, schema or compatibility path is needed.

Prepared parameters retained with the startup request determine whether to bind
identity; notifications and replies share validation/persistence before display.
Provider messages share one forwarding/disconnect path. `ensure_thread_resolved`
checks creation uncertainty at launch and shutdown. Diagnostics use durable
identity/submission evidence, never mutable input presence.

The fixture copies known context sources directly and uses bracketed paste to
avoid native burst timing. Transport reconnection has one explicit marker after
the upstream connection succeeds; no connection counter or separate edited flag.
Exact native rollout input proves draft preservation and no replay, replacing the
weaker model-request substring check. Warning and transport faults remain separate.

## Incident evidence and related changes

The original native log database was read-only. Correlation by process_uuid ties
provider `pid:54609:c02fbf8a-6b4d-48e3-81e9-506349b24dce` and renderer
`pid:55046:38554323-c2ff-45df-bd55-d0c147d2b6be` to the first incident:
thread/start and four thread/list requests at 1792351542, dropped provider messages,
then renderer thread/list reset and worker-channel-closed errors. Native thread
`01a1507a-1c40-7090-a303-3380eea43408` continued work. The second incident starts
1792351557 on unix client.sock under `.tmpwquBMp`. These establish disconnection,
not which relay operation caused it. Original Sessions and native homes were never
used as destructive probes. Snapshots remain `/tmp/loo497-demo-first-history.json`
and `/tmp/loo497-demo-second-history.json`; they confer no current control authority.

Installed 0.14.5 and current source share the initial relay pattern. Native PTY
checks use real native 0.163.0, disposable homes/stores and a loopback model with
external egress denied. No credentials, provider-account changes or shared-store
migration. Plain and hook-wrapped launch/tool/exit/resume probes pass. Native
rollouts must have exactly one conversation, two turns and two tool calls.

The configured `demo` probe copies LOO-497's AGENTS.md, scratch and Infrastructure
Wave text into the isolated repository. Wave context is supplied as scratch;
there is no PM binding or login. Installed 0.14.5 passes it both directly and via
the real cmux wrapper using a nonexistent socket and isolated native home.
That still does not reproduce the reported reset. Live account/plugin configuration
and live cmux tab tracking are distinct from these local protocol checks.

Useful native evidence roots under `/private/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/`:
- `lf-codex-terminal-v5l8y1wx`: initial candidate native plain/hook launch and resume.
- `lf-codex-terminal-o9o8zm5o`: installed configured demo with copied LOO-497 context.
- `lf-codex-terminal-cvvd0rfk`: installed configured demo through the real cmux wrapper.
- `lf-codex-terminal-l5q8z1q0`: compressed v0.14.7 candidate configured demo through the real
  cmux wrapper; both variants retained one native conversation, two turns and two
  tool calls, same-Session resume, clean exits and zero live providers.
Earlier installed probes are documented in `/tmp/loo512-evidence-before-durable-recovery.md`;
none reproduced the incident.

Review fixes: pin submission receipts to the captured input; keep dispatch receipts
inside attachment admission; record rejection before exposing retry input; reset the
bounded drain deadline when renderer loss occurs after a long-running connection.
The renderer-loss regression proves an outstanding first-turn acknowledgement
survives and a reopened Session retains identity without replay.
LOO-508 separately owns terminal-warning output; LOO-324 owns cross-account resume.
Related history: `3316a50613` repaired OpenCode 2 independently; `28dc141671` owns live
Session transport. Infrastructure's LOO-502 decision retains explicit same-Session
retry without automatic replay.

LOO-508 merged in `e204ce6072` (#1595), included in the 0.14.7 base (`3429242a3b`).
The native probe retains its warning-during-draft acceptance. The merged PR records
Linux checks passing; unresolved macOS pending-request and title/reconnect startup
checks remain with gate. Local protocol success cannot establish installed recovery.

The first rebuilt wrapped probe timed out with the edited draft still visible
and no submitted follow-up (`lf-codex-terminal-sphsiucu/wrapped/resume.txt` under
the evidence root above); its warning stayed private. This suggests native
paste-burst timing, not an established relay cause. Explicit paste boundaries
passed both variants without retries or a longer timeout. The failed probe's
identified disposable renderer was terminated; its owner and provider exited.
This does not establish the original incident's cause.

## Completed transport and version comparison — October 18, 2026

The native fixture now has an independent `--transport-failure` case. A byte
proxy on a disposable Unix socket closes the renderer and upstream streams
without a WebSocket close handshake after the PTY displays an edited Unicode
draft. It retains the same renderer process/PTY, observes another connection,
accepts an additional edit, and verifies the exact submitted string in native
history: `typed reconnect follow-up café 雪 🦀 ready after failure`. The rollout
must still contain one thread, exactly two turns and two tool calls, with one
follow-up input. Title-warning coverage stays in the default independent case.
The proxy uses a short `/tmp` socket path to fit macOS's Unix path limit.

Candidate 0.14.7, installed incident-version 0.14.5, and the released 0.14.6
binary all pass the transport case in plain and real-cmux-wrapper variants. Evidence roots under the temporary directory
listed above: `lf-codex-terminal-f47r_5i9` (candidate) and
`lf-codex-terminal-88cujkos` (released 0.14.6), and
`lf-codex-terminal-voaz879r` (installed 0.14.5). Candidate default warning coverage
also passes both variants (`lf-codex-terminal-156ejad1`). The released archive is retained
under `/tmp/loo512-release-0146`; SHA256 matches published SHA256SUMS:
`f477effd39b70239a1c36f127223714cd88c78a09ca3fdae70e261bcbe8aca87`.
Release source comparison `v0.14.5..v0.14.6` attributes the relevant changes to
`28dc141671` and `3316a50613`: history recovery no longer returns emptiness;
resumed launches with a saved identity do not resend the initial prompt. Both
versions still propagate any client-task error out of the listener. They do not
contain this candidate's durable submission receipts or listener-error recovery.

Released 0.14.6 separately fails the default warning case at the private-output
assertion (`lf-codex-terminal-oenfh7_1/plain/resume.txt`), before any transport
fault. LOO-508's later warning fix explains that comparison, not the original
fresh-launch reset. Earlier combined candidate warning/transport checks passed
both variants (`lf-codex-terminal-d3z0hgen`); the independent transport case avoids
making that warning fix a prerequisite for comparing releases.

Further read-only incident inspection found original capture directories
`~/.lf/runs/45/45da789a9098416098b6a89336bfb21b` and
`~/.lf/runs/70/705039678b594919a3d1df9167b60603`. Each retains only the captured
initial input, context/manifest, and a terminal interruption at 19:38 UTC, minutes
after the 19:25 resets. The corresponding Process rows are interrupted with no
saved error; no terminal or relay diagnostic file exists there. The native log
database now starts at 1792352216, after both original resets. The earlier
process-correlated notes remain evidence; an exact failing relay operation
cannot be recovered from these surviving records. No original Session, provider
account, native configuration or checkout was used as a fault target.

## Remaining acceptance — reconciled October 18, 2026

The supplied iteration direction requested draft entry across an actual transport
failure and a released-version comparison. Both now have native evidence; they
are completed local acceptance, not remaining implementation. Read-only inspection
also exhausted the identified original capture and rotated native logs without
recovering the initiating relay error. The repair remains a candidate.

- The original configured reset trigger remains unknown. A configured reproduction
  with private relay diagnostics, or a surviving original error identifying the
  failing operation, is still needed. The transport fault used here occurs during
  draft entry on resume; it does not reproduce the original fresh-start reset.
  All three versions passing that fault argues against claiming an upgrade or
  this candidate establishes incident repair. No substantial implementation repair
  is currently identified; selecting one requires causal evidence.
- Installed/configured recovery remains unproved, including live account/plugin
  configuration and actual cmux tab tracking. Disposable native acceptance does
  not exercise those paths or authorize faulting original Sessions.
- Gate's static checks pass. Compiled affected suites, Claude/OpenCode preservation,
  and Clippy remain deferred to capable CI because local resource preflight failed
  below the repository's emergency disk reserve. Existing build and eight relay
  test evidence remains prior evidence, not a new gate pass. Publication and Task
  completion have not occurred.

Review: the single Session identity/captured-input receipt path matches the
candidate. Native rollout assertions establish exact edited input, one thread,
two turns and two tools. The reconnect marker proves an upstream socket reopened;
rollout and terminal assertions supply the history/submission evidence. No reset
cause follows from these observations. LOO-508's warning fix and Release's
operation-entry lesson retain their separate ownership and evidence limits.

No new product decision was accepted. Changing the requirement to explain the
original reset would change Jack Heart's accepted outcome; it remains unresolved.

## Gate review — October 18, 2026

Reviewed the full HEAD-to-working-tree change, including staged and unstaged work,
against the accepted outcome. No source change was identified. Receipts use the
existing Session observation owner, are scoped to captured input, and precede
dispatch under the attachment fence. Reconnect selects durable identity; uncertain
dispatch never grants replay. The test cases distinguish rejection, lost replies,
notification ordering, overlapping reconnect and stale attachment admission. The
README describes the candidate behavior and TESTING.md describes the independent
warning and transport proofs. Existing Wave edits were preserved without further
changes.

The standard changed-aware runner selected architecture and Rust. Its automatic
resource recovery retained other Tasks' builds and failed preflight with 13.6 GiB
free against the 32 GiB emergency reserve; uv-cache inventory also exceeded its
15-second deadline. No product suite started. Do not bypass this reserve or treat
the infrastructure failure as a product-test result. Full compiled affected tests,
Clippy, Linux transport coverage and disposable installation acceptance remain with
capable CI. The prior native PTY proofs above remain applicable because gate made
no source changes; no new native or installed recovery proof was produced.

Check: `git diff --check`, `cargo fmt --all -- --check`, `uv run python scripts/check_architecture.py`, `uv run --no-sync python scripts/check_rust_test_inventory.py`, and `uv run ruff check` / `uv run ruff format --check tests/e2e/codex_terminal.py` passed; `uv run python scripts/test.py --base HEAD --reuse-passing` failed resource preflight before suites (13.6/32 GiB); compiled Rust suites/Clippy/Linux/installation deferred to capable CI; prior native PTY evidence retained.
