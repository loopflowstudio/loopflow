# Task continuation evidence

Updated 2026-09-25 from the branch for
[Task continuation · LOO-295](https://linear.app/loopflow/issue/LOO-295).
No exact Wave was named or bound for this memory pass; placement remains
unresolved. This repository record does not change a Wave mandate, chapter
plan, or Task status. Branch implementation and bounded demo acceptance do
not establish shipment.

## Accepted model

A loopflow is an ordinary Flow containing backward edges, accepted wherever
a Flow is accepted. Task binding supplies context and execution authority;
it does not change navigation. Ordinary and Task execution share
ExecutionCursor and its transition reducer. Task transactions and ordinary
position-file locks remain separate persistence owners.

Advance and Iterate choose navigation. Blocked requests help through one keyed
Ask running unblock; human Complete returns evidence to the waiting decision
agent for reassessment. Ask readiness cannot release the caller, and Ask
completion does not choose a navigation decision. Retry retrieves the retained
answer instead of opening an identical conversation. An unresolved answer
remains an unresolved blocker.

The pursue body is implement → compress → review-slice → concept-review →
loop-decide, followed by `demo human:true` and a second loop-decide. Human
Complete returns demo feedback and the revised design to that outer decision.
Both loop-decide occurrences have explicit edges to implement; human steps
have no backward edges or navigation verdicts. A failed demo can complete its
conversation with useful feedback; that completion does not claim successful
behavior. Concept review begins with concrete usage, treats product
simplification as valuable in itself, and keeps proposals distinct from
accepted requirements and verified behavior.

Human feedback and proposed next work live in self-contained, topic-named
scratch notes, useful outside the producing conversation or any particular
next skill. Ready summaries carry exact paths plus a takeaway. Loop-decide
starts there, reconciles the current design and other evidence, and cites those
paths in its own direction. No scratch filename grants navigation authority.
Prove this handoff against the actual prepared prompts for both loop-decide
and the following implementation Run: referenced notes and linked design
contents must arrive intact, including nested and untracked Markdown. A path
in a summary alone is insufficient. Fresh Runs receive launch-time snapshots;
decision agents must reread files that may have changed since assembly.

## Continuation proof

When changing Task continuation, prove the controller driver as well as its
cursor reducer: execute multiple finite provider turns, carry review direction
into a second pass, and recover a saved verdict without starting another review.
Keep failure/interruption distinct from crash recovery. An interactive review and a
finished Flow are observable stops; neither is an implied Task completion.

Captured occurrences own human and decision policy. Build continuation fixtures
with actual human Skills, routers and Ops; launch receipts must not supply a
second policy or turn an autonomous fixture into an interactive review.

Backward edges have no pass budget. Iterate may continue indefinitely; counts
describe progress and never force a stop. Keep this true after human revision
and recovery. Blocked is a judgment that help is needed, not an iteration limit.

Use ephemeral stores and simulated provider/PM edges for this proof. A native
terminal handoff or installed-Home demo is separate evidence and must be named
explicitly when it has not run.

Restart can reuse position versions and worker generations. Fence driver
failure handling and next-boundary claims with invocation identity as well;
prove that a late error cannot release the replacement worker's claim.

Capture every XOR alternative and router before execution. A selected cursor
stores its path and child position; the captured parent owns the body. Test
recovery after deleting sources, including pending routes, nested interactive reviews
and returning from a completed child. Never recover an uncaptured historical
body by loading today's catalog. Preserve its bytes and later history, name
the recovery action, and leave unrelated Sessions usable.

Persist autonomous candidates separately from successful Run completion.
Failed or interrupted Runs discard their candidates and lose decision/Blocked
authority. Review completion remains an explicit Session action after provider exit.
Task recovery must check the originating Run before reclaim replaces its binding,
then consume a successful candidate under the original fenced claim. Recovery
fixtures need real completion receipts; an invented Run ID proves only that a
candidate survived storage. Missing or unfinished receipts leave recovery unresolved.
For external operations, cursor settlement alone cannot prove exactly-once
effects across a crash.

Serialize every Ask writer, including reopen reset, under the launch lock and
reread before writing. Persist Complete before native teardown so cleanup
failure cannot revoke an answer. Recover an unpublished failed human launch
only with a readable manifest, no published native identity and no owned live
client; retain the boundary and its evidence. A published native identity must
survive reopening.

Read review feedback from the exact saved position during atomic completion,
not from a caller's earlier copy. Preserve completion before continuation or
cleanup, and carry its summary to the following decision through cursor
direction. The human waived migration of old review conversations for this
redesign; obsolete saved human navigation may require a fresh invocation.
That exception does not waive preservation of other recoverable execution facts.

## Handoff lessons and evidence limits

A Home-local saved ID must reach an executable that reads that Home. The branch
repairs development handoffs selecting PATH's installed artifact by continuing
through the development executable; installed promotion retains its own
selection rules. Test the advertised reopen command through the real CLI,
with another lf first on PATH and binary overrides absent.

The live demo also exposed two distinct context gaps: a decision tool shell
resolved the installed CLI without its Flow/Run binding, and a Ghostty reopen
selected a different native provider Home. Manual binding and Home corrections
enabled the demonstration; automatic propagation is not established by that
success. Compare the environment of the executed terminal command, not only
the process requesting a window.

The recorded credential discriminator found that this checkout's stored Codex
OAuth injection caused native login status to reject an agent-identity JWT;
removing only CODEX_ACCESS_TOKEN restored native login. The branch removes
that stored-OAuth mapping. Its forwarded-account lease path and ambient auth
status/Home consistency still need separate assessment. Do not generalize the
local fix into credential copying or a global native-store override.

The [accepted live demonstration](https://github.com/loopflowstudio/loopflow/blob/f122a264bbf75916df60874966109d67afbd6838/scratch/live-loopflow/final-review.md)
records human acceptance and finished invocation
`41729e12-84b5-4ee8-9ca9-4ae836a20b76`: two fresh writing Runs, Iterate direction,
keyed Ask completion returning to the same decision Run, reassessment, and a
separate final human approval under the earlier model. That staged transport
exercise predates the demo-feedback/outer-decision design and does not validate it. The initial
confusion about its purpose and the manual context corrections remain evidence;
it is not whole-Task, installed-client or full-branch acceptance.

The [integration evidence](https://github.com/loopflowstudio/loopflow/blob/f122a264bbf75916df60874966109d67afbd6838/scratch/cursor-integration.md),
[recovery proofs](https://github.com/loopflowstudio/loopflow/blob/f122a264bbf75916df60874966109d67afbd6838/scratch/saved-xor-recovery.md)
and [boundary compression review](https://github.com/loopflowstudio/loopflow/blob/f122a264bbf75916df60874966109d67afbd6838/scratch/compress-boundary.md)
record focused deterministic tests and static checks. Those results use
simulated provider/PM effects and overlap; do not sum their counts or claim a
new test pass from this memory update. Live Task parity, full CI and hosted UI
acceptance remain unestablished by these records. The commit links identify
local history; remote availability was not verified in this pass.

The later [interactive feedback proof](https://github.com/loopflowstudio/loopflow/blob/b054e93de0ffa21fac6d4890cdac6962ae26bdc8/scratch/interactive-feedback-proof.md)
records ordinary and Task completion/recovery checks and 12 passing Swift
Session store tests for the current Complete action. The
[scratch handoff proof](https://github.com/loopflowstudio/loopflow/blob/b054e93de0ffa21fac6d4890cdac6962ae26bdc8/scratch/scratch-feedback-handoff.md)
records 16 focused checks including actual prompt contents across review,
decision and implementation. These are recorded local results with simulated
provider effects, not a live demonstration of the revised double loop or a
hosted UI pass. No behavioral suite was rerun for this memory curation.

The [final compression review](https://github.com/loopflowstudio/loopflow/blob/b054e93de0ffa21fac6d4890cdac6962ae26bdc8/scratch/compress-model-review.md)
found no further coherent local runtime deletion. Task transactions and ordinary
file locks protect different authorities. SQL cursor projections still recover
legacy flat progress; removing them requires a forward migration. Captured
human Skill tokens still supply synchronous prompt preparation. Retain these
until their responsibilities move with proof; renaming them or deleting their
recovery evidence would not simplify ownership.

## Follow-up boundaries

- Existing choose-one XOR integration belongs to LOO-295. The filed
  [Software UX prototyping and Flow alternatives · LOO-297](https://linear.app/loopflow/issue/LOO-297)
  explores `design → review-design` versus `prototype → review-prototype`,
  and a later demo/concept-review choice. Prototype means runnable interaction
  exploration with hand-built data. Parallel forks, joining and variant
  retention remain design choices, not accepted runtime requirements.
- Broader Task/Linear state restoration remains
  [State restoration · LOO-296](https://linear.app/loopflow/issue/LOO-296).
  This branch's local recovery fixes do not establish that broader outcome.
- Removing the Wave interpreter has a separate
  [deletion directive](https://github.com/loopflowstudio/loopflow/blob/f122a264bbf75916df60874966109d67afbd6838/scratch/wave-playhead-removal.md).
  Preserve cadence, chat, failure presentation, journals and Task shared types;
  do not extend obsolete Wave traversal for parity. Placement and a concrete
  Task remain unresolved here, as does follow-up ownership for the handoff gaps.

Retain scratch until normal delivery cleanup; these curated decisions and
commit-addressed evidence must survive it. No Wave status or chapter evidence
was inferred from historical mentions of Infrastructure.

## CI repair: 2026-09-25

PR #1283 head `e6107b9b5c4d9bf8b520d6a601a813a9988a9b4b` failed Rust
formatting and three Ask-session tests. The tests reached Session listing with
mocked launchers but no `LF_BIN`; listing builds the open command and therefore
still needs executable resolution. An installed development CLI masked this
fixture dependency locally. A direct compiled-test run with executable overrides
unset and only Git on PATH reproduced the exact CI error.

`AskHome` now pins the test executable and restores the previous `LF_BIN` with
the rest of its environment. Keep launcher effects mocked; do not weaken
production executable resolution. Gate Ask-session changes with the clean-PATH
proof described in `TESTING.md`, and run formatting after the final Rust edit.

Local verification passed: all 14 `ops::human_session::tests` with `LF_BIN`,
`LF_CONTROL_BIN`, and `CARGO_BIN_EXE_lf` unset and system-only PATH; `cargo fmt
--all -- --check`; and `cargo clippy --all-targets -- -D warnings`. The first
module run exposed that a Git-only PATH also hides `ps`, needed for native
client ownership checks; restoring system tools while keeping `lf` absent
passed the complete module. Architecture validation passed on a copy of the
current tracked files; an in-place scan had included stale ignored
`.lf/tmp/gate-materialized-source` content. These are local simulated Session
proofs; CI on the published repair remains separate evidence.
