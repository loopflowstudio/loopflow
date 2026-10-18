# LOO-507 — Pi harness

2026-10-17 design, updated after the compatibility repair pass. Jack Heart's Task
directive accepts the outcome below. On October 18, Jack Heart accepted portable
skill-name simplification and authorized shipping after its repair; the earlier
demo evidence and coverage limits remain recorded. Earlier design/evidence is retained at `3a0b71d455`, and the first
published implementation at `806338d501e4d2ac088033764cc0edc1dd48074d`.

## Outcome and boundary

Pi participates in existing Harness, native Session, capture and Task/Flow paths.
One PR includes selection, exact continuation, context/skill delivery, steering,
text/tool/usage evidence, interruption, failures, and Desktop/docs. No registry,
store, cross-harness migration, or credential broker is introduced.

Pi 1.3.0 is the protocol baseline; upstream reference
`5ba1cbaf3d96d204d847c84ce235c62dae23e0fd`. The package's RPC, CLI, extension and
session-format docs and installed implementation supplied the protocol evidence.

## Owners and decisions

- `HarnessKind`, config and prompt validation select `pi[:provider/model]`.
  Pi owns its configured model and authentication. No Claude/Codex account identity
  is inferred from the model. Personal account preferences are recorded separately
  in `pi-authentication.md`; they do not restrict the product's provider support.
- `PiHarness` owns one admitted RPC child using shared launch, attachment fencing,
  dispatch and cleanup. `get_state` supplies the exact absolute conversation file
  used for `--session`; a missing retained file fails instead of starting over.
- Native CLI uses a launch-local extension in LF_HOME. It reports identity and
  events, projects ContextDelivery into each model request, and refreshes after
  start and manual/automatic compaction. Pi keeps system/tool declarations and
  conversation history. No terminal interception or renderer is added.
- One Loopflow turn spans Pi's internal retries, tool calls and queued steering;
  only `agent_settled` completes it. Blank terminal text fails headless execution.
  Pi owns internal retries; a final failure requires explicit later input.
- Direct RPC steering snapshots the initiating turn at acknowledgment. Queued,
  rejected and uncertain delivery are distinct. Settlement racing acknowledgment
  or a lost response stays uncertain; no new turn origin is written.
- Source review found no production consumer of `Harness::send_current` in the
  shared lifecycle. Public Work steering therefore uses the existing provider
  callback/receipt path: Pi's extension polls only during a run and queues custom
  steering in Pi. Omitting `triggerTurn` queues while streaming and only stores
  when idle, so settlement cannot accidentally start more work. Emitted receipts
  remain unconfirmed, independent of the turn result. No dispatcher or store is added.
- Native assistant-message callbacks retain cumulative usage before settlement.
  Existing receipt keys deduplicate observations; recovery rejoins the same usage
  counter. Output/usage/completion retain the initiating turn's Process/Work origin.
  Unknown usage stays unknown; a crash cannot complete unfinished work.
- SkillInvocation remains the skill/argument owner. Task admission and execution
  boundary descriptors remain shared. Pi supplies no provider sandbox: confinement
  belongs to the caller's OS environment. Managed accounts, subscription estimates,
  response schemas, turn limits and cross-harness import remain unsupported.
- OpenCode PR #1590 landed at `3316a5061357d452c097a22cae15b8fccf1ef8d9`.
  `lf sync --manual` consumed it in `614c23816b`; the launch conflict preserves Pi's
  native arm and shared Codex/OpenCode `SessionCommand.launch`. Pi's failed-start
  test exercises the delivered shared recovery owner without duplicating it.

## Delete — do not maintain

No remaining deletion targets. The surviving path replaces:

- Pi's participation in the generic synthetic outer retry loop. Keep internal
  Pi retries and explicit continuation in the saved conversation.
- Assistant-count-only success. Require a nonblank terminal answer while keeping
  valid tool-only intermediate messages and failed/interrupted outcomes.
- `before_agent_start`-only context injection and its old callback name. Use
  request projection after automatic as well as manual compaction.
- Settlement-only native usage. Persist cumulative message checkpoints through the
  existing store; do not add a second billing stream or settle partial work.
- Pi's inherited `NotSteerable` implementation. Preserve direct receipts and
  deliver public Work steers through the current provider callback owner.

## Review findings

The simulated code review found that a direct RPC adapter method alone would not
connect public steers; the extension now owns that integration. It also found an
idle-Harness regression in ownership validation and an ambiguous malformed RPC
acknowledgment; idle sends remain NotSteerable and malformed acknowledgments remain
Unknown. Request construction no longer clones a potentially large prompt merely
to retain its command name.

The runtime probes corrected two fixture assumptions: raw event rows inherit
ownership through the shared history reader, and scratch must be edited at the
Session's recorded checkout after repository promotion. Neither needed a new
production owner. Task fixtures now explicitly use canonical checkout paths.

The reduction separates RPC startup from stream consumption, shares the extension's
context callbacks and fatal shutdown, and computes usage-recovery stream/sequence
metadata once per turn. Runtime probes share one model/temporary-home lifecycle.
Crash completion still emits its failure even if history retention fails; normal
stream events retain their stricter write-before-delivery path. Combining those
paths would obscure their different failure contracts.

The review also found that a malformed steer acknowledgment with `queued` but no
`success: true` could count as delivery. The adapter now requires explicit success;
the existing uncertainty fixture covers that combination. Compatibility notes
retain the original counterexamples by git reference and current acceptance gaps.

## Remaining work

The October 18 interactive demo supersedes the implementation-complete conclusion
for skill discovery: Jack Heart observed extensive Pi warnings for namespaced and
long synced skill names in the real shared catalog. See [demo feedback](pi-demo.md).
Native export naming is corrected while retaining canonical identity/source/assets.
The actual Pi loader now reports zero warnings on the shared catalog, with both
long synced PDF variants retained; see the demo note for preservation evidence.
Jack Heart accepted shortening native names and removing redundant aliases while
preserving skill behavior, then authorized `lf ship` through verified main landing
after this repair. Use one portable export per source; preserve original `lf`
identities and suffix collisions instead of dropping skills. Delete vendor-specific
slash alias projection and its exclusive assertions. Retain third-party preservation,
resource paths, native declarations and safe replacement pruning.

Implementation repairs are complete. Reproducible provider-isolated scenarios and
coverage boundaries live in `scripts/benchmarks/pi/README.md` and `probe.py`. The passing
managed Task/Flow proof includes JSON routing, only the selected branch and nested
`lf` Work/cwd. The real Pi PTY proof includes Unicode edit/resize and exact resume.
The real overflow proof sees changed scratch on the first recovered request;
public live steering retains one initiating turn and Process with an independent
emission receipt. Native partial usage and before-identity recovery have focused
integration coverage. These isolated probes are not account or visual acceptance.
A separate candidate headless Loopflow request through Jack Heart's selected
OpenCode/Gemini 4 Argon route passed with tools disabled; rendered interaction
remains unreviewed.

Gate owns affected suites and the remaining comparison matrix: real manual and
threshold compaction, active viewer handoff, real tool crashes/child cleanup,
repeated partial recovery and real 429/5xx/invalid-model cases. Demo owns rendered
judgment and the selected personal account route. Jack Heart
authorized ship and Task completion after the naming correction and verified merge;
remaining automated acceptance belongs to its gate.

Recovery after interruption: implement and compress succeeded in Flow
`fce325b0-3db9-4fa6-8196-ae13ed06dc7a`; sync left no exit record, but its conflict
agent completed merge `f605333f08`. Fresh Process and OS reads found no live Flow
or sync agent. Caller edits survived in stash `1c80394799ca283911a7668c063e33bf30f9a095`
and were reapplied without deleting the stash. The TESTING.md conflict preserves
main's removal of historical OpenCode evidence and retains the maintained Pi
probe under `scripts/benchmarks/pi/`, matching the new generated-review exclusion.
Reconciliation and publication continue in the primary conversation; no new
implementation Flow is needed.

Post-sync recovery checks: four Pi launch-contract tests, formatting, all-target
Clippy, context-hook regression and Python lint passed; no Pi implementation bytes
changed from the verified stash. The configured Loopflow account probe also passed.

Checks: `cargo build -p loopflow --bin lf`, `cargo test -p loopflow --lib pi_` (21), `cargo test -p loopflow --test session_launch_contract pi_` (4), `node scripts/benchmarks/skill-invocation/test_pi_context.mjs`, all six `probe.py --pi /tmp/loo507-pi/node_modules/.bin/pi` scenarios, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `uv run ruff check` / `ruff format --check scripts/benchmarks/pi/probe.py`, and `git diff --check` passed; Rust tests/probes used `scripts/test_network.py` with inherited `LF_*` cleared and stdin closed. Existing managed Task/Flow routing proof remains applicable. `lf context` is within both budgets; remaining affected suites and acceptance belong to gate/demo.

Naming repair checks: `cargo test -p loopflow --lib skills::` under `scripts/test_network.py` passed (26 tests); `cargo fmt --check` and `git diff --check` passed. Binary build and real-catalog Pi discovery passed (59 diagnostics → zero, 27 third-party files unchanged); `cargo clippy --all-targets -- -D warnings` passed.
