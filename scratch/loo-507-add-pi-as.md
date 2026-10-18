# LOO-507 — Pi harness

2026-10-17 design, reconciled after implementation and compression. Jack Heart's
Task directive accepts the outcome below; implementation choices remain draft
pending demo review. No new product decision was accepted in reconciliation.

## Outcome and boundary

Pi participates in the existing Harness, native Session, capture and Task/Flow
paths. One PR includes selection, exact continuation, context/skill delivery,
text/tool/usage evidence, interruption, failures, and Desktop/docs. No new registry,
store, cross-harness migration, or credential broker.

Protocol reference: earendil-works/pi commit
`5ba1cbaf3d96d204d847c84ce235c62dae23e0fd`, coding-agent package 1.3.0.
Primary references under `packages/coding-agent/docs/`: rpc.md,
rpc-commands.md, json.md, cli.md, session-format.md, security.md.
An isolated installation of 1.3.0 passed RPC and native non-TTY launch,
context delivery and exact resume against
a local model fixture; no real account or rendered terminal has been demonstrated.

## Owners and implementation

- `HarnessKind`, config and prompt validation select `pi[:provider/model]`.
  Pi chooses its configured model when omitted. Pi owns authentication; no
  Claude/Codex account identity is inferred from Pi's underlying model.
- `PiHarness` owns one admitted `pi --mode rpc` child using existing launch,
  attachment, fenced dispatch and cleanup. `get_state` supplies exact native
  session identity. Persist an exact session file token for `--session`; fail
  when a retained file is missing rather than silently creating a conversation.
- Native CLI uses a launch-scoped extension to report the session file and
  refresh existing ContextDelivery at start/compaction/run boundaries. The same
  extension works with RPC; no renderer or terminal interception.
- Map message deltas, authoritative messages, tool lifecycle and completed
  assistant usage to existing ConversationEvents. One Loopflow turn spans
  Pi retries and tool calls; only `agent_settled` completes it. Missing usage
  fields remain None. Auth errors and EOF cannot report success.
- Keep skill translation through SkillInvocation; preserve source/arguments.
  Shared Session history records output and usage under exact turn origins.
- Task admission includes Pi alongside Claude/Codex. AgentExecutionBoundary
  records the checkout and Git/control roots; docs/security.md assigns containment
  to the OS. Pi uses that existing execution model. Its cwd and project trust
  provide no confinement. Explicit provider-only Worktree scope without
  a managed boundary remains unsupported for Pi. Managed account switching,
  subscription/rate windows and remote credential delivery remain unavailable.

## Review findings and implementation decisions

- Capture identity is the absolute Pi session file, not the short UUID; missing
  files fail before Pi can silently create a replacement conversation.
- One normalized turn spans retries and multiple model calls. Final output is
  recorded once; cumulative usage preserves unknown values and recovery rejoins
  the existing counter instead of creating another billing stream.
- Shared admission, attachment fencing and process-group cleanup own RPC
  execution. A stale attachment cannot send input or stop its successor.
- Native callbacks handle idle dialogs without requiring an active turn.
  RPC dialogs, extension failures, malformed events and EOF fail visibly.
- Review corrected a fixture trace identifier, moved generated extensions out
  of the checkout into LF_HOME, and retained the existing one-shot Run lifecycle.
  Same-Pi resume uses persistent Sessions; no cross-harness history transfer.
- Recovery review on 2026-10-17 corrected the native probe: initial input uses
  `lf -i -a pi:fixture/proof : TEXT`; continuation uses `lf -i resume ID` with
  piped Pi input. `--message` belongs to headless connection. Real Pi retained
  the same file and recorded two model requests and two usage receipts.
- Compression removed the native replay's discarded intermediate records and
  the RPC adapter's full retained AgentConfig. Only settlement records native
  output/usage; only the attachment survives launch. Acquire ownership before
  taking stdin or clearing the child so failed cleanup remains retryable.
  Claude and Pi share immutable hook-file persistence; headless exit handling
  reuses the shared terminal-error predicate. Mapping internals stay private.

## Reconciliation evidence — 2026-10-17

The checkout and locally available `origin/main` both start at `616d73b4dc`;
there are no additional upstream commits in that ref. No remote freshness claim
is made. Incorporated LOO-499 connect/resume, LOO-415 attachment ownership,
LOO-501 native diagnostics and LOO-502 stable CLI dispatch remain the shared
owners; Pi adds no parallel lifecycle or pinned executable. Infrastructure and
its immediate Release child's goal/memory were read. Release's entry-point
lesson applies: adapter proofs alone cannot establish composed Task/Flow behavior.

Source review found no bounded implementation correction. Existing tests prove
RPC interruption, auth rejection, crash/EOF, missing files/binary, stale attachment
rejection, usage recovery, public headless skill/context delivery and native
callbacks. Swift evidence proves four-harness Session switching. The real Pi
probes use a local model fixture; native resume retained one file across two
requests and two usage receipts. Those results do not prove account access,
terminal rendering, or every extension/context boundary.

## Remaining work

The adapter and selection surfaces exist; acceptance remains open. Gate owns
the affected suites and full matrix, including existing-harness regression
coverage. Explicit gaps are a composed managed Task/Flow with Pi, a nested `lf`
invocation retaining Work identity/cwd, and context refresh after compaction.
The boundary-root assertion and captured caller environment do not establish
those composed behaviors. Missing acceptance coverage remains work, not a waiver.
Demo owns authenticated model access and rendered Pi terminal judgment.

Managed account identity, subscription estimates, response schemas, turn limits,
provider-only worktree sandboxing and cross-harness import remain unsupported
and documented. No new product decision is needed for the accepted outcome;
review still owns acceptance of the draft implementation choices.

Checks: `git diff --check` passed; `lf context` confirms memory/scratch within both limits; reused post-compression passes for `cargo test -p loopflow --lib pi_` (18), `cargo test -p loopflow --lib hook_identity_is_shared` (1), `cargo test -p loopflow --test session_launch_contract pi_` (2), `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (tests cleared inherited `LF_*`, closed stdin); Desktop/selection and real Pi fixture evidence retained, remaining acceptance/full matrix to gate and authenticated/rendered experience to demo.
