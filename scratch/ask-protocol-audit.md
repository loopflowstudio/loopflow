# Ask protocol audit — LOO-295

Integration disposition from main: all three findings below are now repaired.
The original source findings remain as evidence. Deterministic Ask concurrency
and cleanup-failure proofs pass, and a real CLI regression reproduced then
repaired the development binary/Home mismatch. Exact runs and evidence limits
are recorded in saved-xor-recovery.md. No live provider acceptance is implied.

2026-09-25. Bounded source review of Blocked → keyed Ask → unblock → human
Complete → returned evidence → loop-decide reassessment. No runtime, docs,
skills, or existing human-review artifacts were edited. No tests, providers,
Sessions, installations, PM operations, commits, pushes, or delegation were run.
The reproducing tests below are proposed tests, not reported executions.

Read `scratch/protocol-review.md`, the Ask contribution notes, current
`docs/lf.md` (Flow decisions and recovery), `docs/authoring.md`, the architecture
ownership descriptions, and the canonical loop-decide/unblock skills. Some
historical statements in protocol-review are superseded by current source;
in particular there is no pass limit. This review does not reopen the active
human concept review or assess the whole branch.

## Findings and final source disposition

### 1. Repaired during audit: Ask reopening could overwrite a completed answer (P1)

Final reread: `open_boundary` now acquires the launch lock before resetting the
record, rereads Waiting status and the observed Run ID under that lock, and
retries if the binding changed. This closes the lost-answer interleaving below.
The original finding and smallest regression are retained as review evidence;
it is not an outstanding persistence defect. No test was executed here.

Location: `rust/loopflow/src/ops/human_session.rs:760`, `open_boundary`, especially
the read/resume/reset/write sequence at lines 761–776. The launch lock is first
acquired at line 778. `complete_ask` and Ask `mark_ready` take that lock, but this
earlier write does not.

Concrete interleaving:

1. Open reads a Waiting record with a published Run ID and ready summary.
2. Its native manifest is unavailable, so `resume_native_run` returns false.
3. Concurrent Complete locks the same Ask, persists Completed with the summary,
   and returns. A missing native manifest does not prevent that completion.
4. Open clears its stale copy's Run ID and ready summary and writes the entire
   Waiting record over Completed, before acquiring the lock.
5. The subsequent under-lock reread sees Waiting again. A new launch may start;
   even if that launch fails, the retained answer has already disappeared.

This breaks keyed recovery: a duplicate caller can wait again instead of
receiving the completed answer. It can also overwrite a concurrent readiness
update. The separate fast path that resumes a published native Run is likewise
outside the launch lock; the late under-lock branch does not recheck Waiting
before attempting native resume. The completed-state check in `serve_ask_locked`
does not protect these earlier paths.

Smallest regression: extend `ops::human_session::tests` using `AskHome`, a
retained Waiting record with a nonexistent Run ID and a ready summary. Pause
`open_boundary` immediately after the missing-manifest result using a test-only
barrier; run real `complete_ask`; release Open and stop before any provider
spawn. Assert the saved record remains Completed, `wait_for_ask` returns the
original answer, and no fresh Session launch is requested. A second ordering
can complete while Open is waiting for the launch lock. Do not use a real
provider or timing sleeps to reproduce this race.

This is an existing reopen path exposed to the retained keyed result; the
new locks around readiness/completion alone do not serialize every Ask writer.

### 2. Repaired during audit: a failed ordinary Flow boundary could authorize Blocked (P2)

Final reread: `require_active` now additionally requires `!run.finished` and
`run.failure.is_none()`. The failed/interrupted cases in the existing recovery
test also assert that it rejects the old Run. The finding below describes the
earlier inspected source, not an outstanding defect. The new assertion was
read but not executed in this review.

Location: `rust/loopflow/src/ops/flow_run.rs:291`, `require_active`; caller:
`rust/loopflow/src/lf/commands/flow.rs:189–200`.

`require_active` checks token, bound Run, deciding/nonhuman kind, and
`!active.completed`, but does not reject `run.failure`. Both
`finish_boundary(token, Some(reason))` and recovery of a failed/interrupted
provider retain the boundary and its Run ID, leave it uncompleted, and set the
failure. The old token/Run therefore still obtains the Ask key after failure.
The same record correctly rejects `record_decision`, which checks failure.

A late command from that attempt can create/join an unblock Session after the
ordinary driver has stopped on a recorded failure. It cannot approve a human
gate or settle navigation, but it retains an external human-handoff capability
that the failed attempt should have lost. This is distinct from retry: retry
clears the failed active boundary before a fresh attempt is bound.

Smallest regression: add an assertion to
`ops::flow_run::tests::recovery_requires_success_and_rejects_stale_or_conflicting_decisions`.
After its real CaptureHandle is finished as `failed` or `interrupted` and
`recover` has persisted the failure, call `require_active` with that same token
and Run ID. It currently returns `Ok(key)`; require an error and byte-equivalent
saved state. Also check `finish_boundary(..., Some("failed"))` directly. This
needs no Ask launch, registry transport, or provider.

### 3. A development handoff can switch artifact and Home before reading the saved ID (P2)

Locations: `rust/loopflow/src/engine/process.rs:268` and `:327–366`;
`rust/loopflow/src/ops/human_session.rs::launch_ask`, `serve_ask_locked`,
`human_open_argv`; `rust/loopflow/src/ops/flow_run.rs:78`;
`rust/loopflow/src/store/mod.rs:233–260`;
`rust/loopflow/src/machine_install.rs::selection_for_current_executable`.

Concrete setup: run the checkout's uninstalled development binary A with
`LF_BIN` and `CARGO_BIN_EXE_lf` absent, while PATH's `lf` is the machine entry
gate for installed artifact B. A saves an Ask or ordinary position in A's
development Home. `resolve_current_home_lf_binary` selects PATH before the
running executable. Both Ask launch and ordinary continuation consequently
invoke B, while their parent computed the launch context from A.

For an active installed artifact, `current_home_lf_home_dir` and
`current_home_database_path` use that artifact's installation selection before
the supplied environment. B therefore looks in B's store/Home for A's ID.
The Ask server fails to read the record, or the Flow continuation fails to read
the position. The tmux launch can already have returned success; an Ask caller
then only polls its still-Waiting record. Generated `human_open_argv` also
selects B and carries no explicit saved Home. `scripts/dev-lf` does not pin
`LF_BIN` to the binary it just built.

This is a specific development/installed-artifact mismatch, not a claim that
normal same-installation resume always fails. Choosing the current installation
instead of an obsolete control pin is intentional; the defect is passing a
Home-local ID across that selection without ensuring its state follows. Merely
restoring an arbitrary historical `LF_CONTROL_BIN` would not resolve that model.

Smallest reproducing proof: in an isolated process/environment, point PATH at a
fixture `lf`, clear both binary overrides, and assert the resolver selects it
rather than A. Pair that with the existing machine-install fixture machinery:
authorize B with store B, supply A's `LF_HOME`, and observe that record lookup
still selects B. A process-level regression can capture the real non-test tmux
argv/environment with a fake tmux executable, then use a probe child instead
of a provider to report its selected record path. Require the launched path to
resolve the same saved ID. No real installation or production Home is needed.

The source establishes this conditional mismatch; this review did not execute
the two-artifact reproduction or inspect this machine's install receipt. The
unit-test `launch_driver` implementation only reads the record and returns Ok;
Ask tests simulate `start_durable_session`. Neither proves the production
artifact/Home handoff. Several fixtures explicitly set `LF_BIN`, masking this
particular setup.

## Trace and authority that do hold in the inspected source

- `lf/commands/flow.rs::control("blocked")` requires a nonempty reason and an
  active Run. Ordinary Flows use `LF_FLOW_STEP` plus the saved bound Run; Tasks
  require the current worker Run, a nonhuman position, and a repeat occurrence.
  Key construction includes invocation and the recursive cursor boundary key.
  `ExecutionCursor::boundary_key` includes index, iteration and selected nested
  paths. A worker retry at the same boundary deliberately reuses its Ask.
- `human_session.rs::ask_once` hashes the key, captures the first question,
  selected `unblock` name, cwd/model and available Work attribution, then
  rereads under the launch lock. Failed launcher startup retains the record.
  Concurrent callers join the same named launcher or published native Run.
  Completed keyed records return the saved summary without reloading a skill.
  Ordinary prompt-only Asks retain their removal behavior.
- The detached launcher clears Flow/Session execution authority, carries the
  saved parent Run attribution, and invokes `session serve-ask`.
  `serve_ask_locked` rejects Completed and does not replace a published Run.
  Fresh startup uses `--tui --model … --__cwd … [--as …] skill unblock …`.
  Native reopening uses the existing provider history and saved account through
  `resume_native_run`/`resume_session_with_env`, not a new decision Run.
- The selected skill name persists; its body is loaded on a fresh launch.
  This is not an immutable snapshot of unblock. Native resume retains the
  original conversation. That documented distinction is not a defect by itself.
- Ask readiness checks the bound Session Run and Waiting status under lock.
  Ready does not release the caller. Complete requires the ready summary.
  `wait_for_ask` returns only Completed, retaining keyed results for retries.
- The Blocked CLI prints the returned summary and explicitly requests
  reassessment. It does not synthesize a verdict, settle a cursor, start a fresh
  loop-decide Run, or approve another gate. The waiting decision agent receives
  the tool result and chooses its next action. After an interrupted attempt,
  the new decision Run must call the same Blocked protocol to retrieve it.
  Repeated calls after completion return the same answer rather than opening
  new Asks. The skill tells the agent to report an unresolved blocker instead
  of cycling; there is no independent semantic test of whether a human answer
  resolved the problem.
- Ordinary `record_decision` rejects foreign/stale Runs and human boundaries,
  requires evidence, and rejects conflicting candidates. Failed-provider
  recovery discards candidates. Task decisions retain store/claim authority.
  Human Flow decisions have their separate exact boundary endpoint. Completing
  the Ask itself changes neither authority.

## Acceptance ordering changed during this review

The first source read had `complete_ask` stop the native Run before persisting
Completed. A later read observed a concurrent fix at
`ops/human_session.rs:484–492`: it now saves Completed first and logs teardown
failure without retracting the answer. The earlier ordering concern is **not
an outstanding finding**. No runtime proof of that fix is claimed here.

Smallest focused proof: create a ready retained Ask bound to a real
temporary Run manifest with a malformed `provider-clients` receipt. Completion
must remain durable and `wait_for_ask` must return its summary despite teardown
inspection failing. This uses no live client and checks the consequential
result, not calls to a stop mock. The final reread found a new
`ask_completion_survives_native_cleanup_failure` test for this scenario; no
execution result is claimed by this audit.

Task human decisions similarly persist their exact transition before relaunch
and provider stop (`human_session.rs::decide`). Ordinary human decisions persist
the verdict before requesting `launch_driver` and stopping the provider
(`flow_session.rs::decide`). A launch failure leaves their saved decision and
recovery path; it is not a second approval requirement. These source properties
do not prove that the selected continuation binary successfully resumed it.

## Evidence limits

No live configured Blocked → native unblock → desktop Complete → reassessment
trace was collected. That is missing acceptance evidence, not an additional
code defect. Existing tests cover simulated launch failure, duplicate callers,
retained completion, missing native manifests, late readiness, prompt selection,
and ordinary result fencing; their names and assertions were read, not rerun.
The pre-publication native process crash window remains acknowledged in the
existing Ask proof; this audit does not claim exactly-once provider execution.

Final reread source fingerprints (concurrent checkout, SHA-256; findings 1 and
2 were repaired between the original review and this reread):

```text
human_session.rs c992ef07ae4e37d2afc981ac3184c573f9a748518e63b254c65a0c1096015169
flow_run.rs      ac19350511faccce8f7f2cb8bdd15f1a261b382e7949501d6416c64345b85514
process.rs       2ae2f00aaed4c48723cf9ca4b7af15f7c47b2ad9afdca5812b0315310460d373
commands/flow.rs d5982364088de8f10fc1272e73d5da12709a4eb9478c030271b6def24128be6b
```

Line numbers in repaired findings describe their original reads. Symbols and
the final fingerprints identify the later inspected behavior if main's fixes
move it again. Only this audit note was written.
