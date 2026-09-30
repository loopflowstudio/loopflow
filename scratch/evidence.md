# Evidence for LOO-298

2026-09-30 · Jack Heart requires code-complete implementation, his demo, resolved
findings and final gates before verified merge. The current feature Flow has no
separate slice or concept-review step. Publication is a checkpoint. This ledger separates current proof from configured acceptance.

Prior results, failures and exact log references remain in
[the preceding committed ledger](https://github.com/loopflowstudio/loopflow/blob/d6dc8c43b73872b8b6e71b0996468d2432b23102/scratch/evidence.md)
and [handoff](parallel-execution.md). This replaces chronology, not obligations.
The complete [remaining matrix](remaining-work.md), [import contract](compress-history.md),
[Chapters](chapters.md) and [native tradeoff](native-turn-retry-tradeoff.md) remain binding.

## Mechanical Flow step checkpoint

Mechanical boundaries execute as real child lf processes through the shared
Flow driver. Existing Flow start/result events name the child's Exec; the driver
consumes the selected result. Public subprocess proofs retain earlier success
across a later failure and resume a killed driver without replaying a surviving
step. Task stop/status preserve that selected step until its exact process dies;
missing process evidence stays unresolved. [The working design](exec-per-step.md)
retains agent-step consolidation and the remaining ownership cuts.

The initial full materialized matrix (`.lf/tmp/cut-i/exec-step-full.log`) ran all
2,032 tests: 2,030 passed, two failed, 16 skipped. One simulated worker bypassed
startup claim adoption and timed out; its fixture now adopts an actual sleep
process identity before driving the simulated provider. The landing proof still
expected in-process operation ancestry; it now checks driver → step → agent-issued
rebase and assigns repair/history to the step. That stronger assertion exposed
another fixture gap: its fake Codex invoked rebase before receiving the thread's
tool environment. It now executes during the simulated turn with its recorded
caller identity. The focused landing proof passes (`exec-step-land-repair.log`).
The repaired startup/stop checks pass seven tests with one nextest leaky-handle
diagnostic (`exec-step-repairs.log`, which also retains the intermediate landing
failure). No configured-provider proof follows.

`exec-step-full-green.log` passes the full isolated materialized rerun: 2,032
passed, 16 skipped, none unrun, fail-fast disabled, no leak diagnostic. Rust and
test bytes match `.lf/tmp/cut-i/exec-step-source.json`; the failed snapshot remains
in `exec-step-first-source.json`. `exec-step-publish-clippy.log`, formatting,
diff checks and architecture coverage pass. The source and materialized tests use
private stores, real lf subprocesses, and simulated provider/GitHub effects.
Swift is unchanged; its prior 291-test discovery pass is retained, not rerun.
No installed Home, configured provider or rendered Desktop acceptance is claimed.

## Discovery checkpoint

`discovery-full-green.log` passes the complete isolated materialized Rust matrix:
2,029 passed (one slow), 16 skipped, none unrun, fail-fast disabled.
`discovery-swift-green.log` passes all 291 Swift tests. All-target Clippy, formatting,
migration history, architecture and generated HTML checks pass. Rust/Swift/test
bytes match the full-run source receipt `.lf/tmp/discovery/source.json`.

The preceding full Rust run had 2,027 passes and two new fixture setup failures:
a completed Flow lacked ended_at, and a parent-filter fixture tried to mutate
immutable parentage. Corrected fixtures pass focused checks and the full rerun.
Swift's intermediate array mocks and hidden-row click failures were repaired;
its final suite includes both hosted MockWaveFixture failures from `142314682`.
These are local repairs, not a new hosted result.

[Discovery design](discovery.md) records public paging/template collision proofs,
5,000-row whole-command timings and mounted terminal retention across partial,
failed and successful refresh. Providers are scripted; terminal proof uses cat.
Configured acceptance and the deepest-first [remaining work](remaining-work.md)
stay open. The next cut gives each Flow step its own actual Exec.

## Published evidence

At `d6dc8c43b`, hosted run `36629797823` passes Rust, Swift, UI, Python, website,
end-to-end, installation, migration and static checks. Scratch-clear and its
aggregate remain red. The earlier non-fail-fast materialized run was 2,001/2,013;
its twelve failures and focused repairs remain in the preceding ledger. The new
hosted pass supersedes the missing-green-matrix limitation, not those observations.

Typed Flow results are published. Native schemas constrain decision and router
outputs; exact selected successful completion owns settlement. Source/canonical
proofs cover invalid→valid, bounded exhaustion, stale success and runtime children.
Public OpenCode uses a scripted provider. Real Codex uses local synthetic Responses
for correction, exhaustion and delayed removed-command rejection; that closes the
legitimate retry failure within that fixture only. Claude correlation has a native
message fixture. No configured Claude or configured-account acceptance follows.

## Captured-event CI repair

Hosted `243e3edee`, run `36633600273`, passed Swift but Rust stopped at the
stalled-status fixture with 907 tests unrun. Production and Rust/Swift fixtures
now name the selected Session event. The first complete, non-fail-fast isolated
materialized run exposed five more obsolete Flow assertions and two fixtures
relying on ambient Git. Those fixtures now own their Git context; Flow tests
resolve the real Session from captured history, retaining binding, Task identity,
rename, open and complete-history assertions.

`captured-ci-full.log` records 2,009 passes / seven failures / 16 skipped.
All seven repairs pass in `captured-ci-repairs.log` (21 tests). The final entire
matrix passes **2,016 tests, 16 skipped, none unrun** in `captured-ci-final.log`,
with one nextest leaky-handle diagnostic on the prepared-input timing fixture,
followed by all-target Clippy; formatting passes too. All logs are under
`.lf/tmp/cut-i/`. This is a disposable materialized copy with development provenance
and isolated data, using simulated provider/GitHub effects where applicable.
The current Swift Task Flow proof passes within the separate in-progress history
consumer build; that does not establish configured Desktop acceptance.
No new hosted result or whole-design completion follows from these local checks.

## Captured-event cut

Logs below are under `.lf/tmp/cut-i/`. These are source/disposable-copy checks,
with simulated providers where provider effects are exercised.

| Proof | Result and scope |
| --- | --- |
| `captured-public-proof-2.log`, `captured-repaired-proof.log` | Public suite 46/52 passed, six failed; all six repairs pass in the later 27-case proof. Retains Task/taskless continuation, Session/Ask replacement, binding, native receipts and indexed metadata. No claim of a fresh whole-suite rerun. |
| `captured-preservation-proof.log` | 207/215 passed, eight failed. Found reselectable old captures and missing capture Exec/Work projection, plus stale fixture columns/counts and inherited database pinning. These failures remain recorded. |
| `captured-preservation-repairs.log`, `captured-last-preservation-repairs-2.log`, `captured-final-repairs.log` | Composed repairs pass: publication interruption retains the same Ask/question/event, bind retains old attribution, final answers/account receipts survive, populated history keeps exact SQL/Started/native sequence, and live blocked feedback uses its owning database. Final four-case repair passes. |
| `captured-replay-proof.log`, `captured-replay-session.log` | Twelve Rust DTO/reservation/replay checks pass; the strengthened public replay consumer additionally passes using Session identity distinct from the artifact key. |
| `captured-swift-dto-2.log` | Sixteen Swift DTO checks pass after removing SessionRecord.run_id and converting capture references. The Monitor continuity assertion now compares Session identity. No rendered Desktop claim. |
| `captured-canonical.log` | Twelve materialized checks pass: populated draft/released upgrades, unknown SQL, native output settlement and runtime-child preservation. Dedicated build target avoids mixing authoring and canonical schemas. |

`captured-unknown-selection.log` and `captured-canonical-unknown.log` additionally prove an old Flow selector without
conversation membership is archived exactly and leaves current capture unknown.

Canonical source hashes are in `canonical-captured-event-source.json`; the copy
contains tracked and untracked source bytes. `captured-canonical-chapters.log` passes all 15 Chapter checks (one 68-second
rotation matrix). `captured-final-static.log` passes all-target Clippy; formatting,
whitespace and architecture coverage pass. Reuse unchanged evidence; do not rerun
whole suites merely because a lifecycle phase changed.

Concrete review findings fixed in this cut: publication compares a capture sequence;
replacement must append and cannot reactivate an earlier event; capture history
projects its own Exec/Work; a failed artifact publication leaves the reservation
recoverable; old unclassified Flow selectors and failure JSON retain original
bytes in `import_evidence`. No migration fabricates a Session from a provider label.
Historical artifact paths and prepared→launching ownership remain intact.

## History reader integration

The released proposal now uses captured event identity and typed Session history;
RunSnapshot and string-subject joins are removed. Native receipts without a capture
or start remain discoverable with unknown attribution and partial usage. The
subtract-only pass removes the completion wrapper, redundant Work tuple and Swift
input-identity wrapper. Original SQL/manifest time and attribution precede import
metadata; provider and recorder outcomes remain separate.

Logs are under `.lf/tmp/cut-i/`. `history-full.log` ran the entire materialized
Rust matrix without fail-fast: **2,013 passed, six failed, 16 skipped, none unrun**.
Four consumer expectations were stale; two import cases exposed overwritten
historical time/source. All six repairs plus bounded-history and orphan-discovery
checks pass in `history-matrix-repairs.log` (8/8). Review also reproduced an
unlinked turn borrowing another turn's Work filter (three rows instead of one);
`history-orphan-scope-red-exact.log` retains the failure and the eight-case repair
includes its correction. **Final `history-full-final.log`: 2,019 passed, 16 skipped, none unrun**,
followed by formatting and all-target Clippy. The exact disposable source receipt
is `.lf/tmp/history-event/source.json`; source/test hashes still match.
`history-docs-final-2.log` records regenerated website docs (the preceding
invocation named a nonexistent helper and did not change files).

`history-public-orphan.log` proves public runs/usage discovery, partial counters
and no borrowed post-bind Task ownership with a scripted OpenCode provider.
`history-budget-proof.log` proves pre-decode limits, retained unfinished entries,
complete exact-caller reads and visible exact-detail corruption. Swift's initial
39 selected checks had two stale expectations; their repaired DTO/Task-history
selection passes 18/18 in `history-swift-repair.log`. Nine lifecycle-scorecard
Python tests, architecture coverage and the prior all-target Clippy pass. These
are local fixture proofs; configured-provider/Desktop acceptance remains open.

## Limits retained at the finish line

- The released-populated public import bridge and its canonical-copy counterpart
  remain required. Schema fixtures are not that bridge or a backed-up real-Home
  conversion. Keep source artifacts, unknown membership/Exec identity, controller-
  and SQL-only evidence, Started, ancestry and exact replay until it passes.
- Native-only aggregate discovery has local fixture proof above. Saved-Flow
  inventory and Desktop paging/reconciliation remain the next implementation cut.
- Configured accounts, rendered Desktop continuity, complete managed/provider
  recovery and the real-Home procedure remain explicit obligations. Never run this
  branch against the installed Home or promote it before authorized delivery.
- Unknown mechanical completion requires inspection before retry; no exactly-once
  external-effect claim follows. Released-claim/live-child settlement and the
  original missing Task/PR publication cause remain unresolved incident evidence.
- Chapter fixtures preserve Task identity, exact Started, worktree, PR and capture
  across rotation and second-Home sync. They prove neither live Linear rotation,
  absence of another Home's work nor a distributed transaction.
- Dense metadata SQL timings were warm synthetic measurements. End-to-end cold/warm
  history/discovery measurements remain owed; earlier 314–339ms Exec p50 readings
  had uncontrolled OS caches and do not establish final product latency.

Jack Heart resolved prospective attribution and structured blocked output on
2026-09-30. The earlier assumptions are superseded; no slice establishes Task
completion or shipment.

## Common Task context and PATH steps · 2026-09-30

Working slice after `a35af5223`; Jack Heart's subsequent decisions are retained
in questions.md and [the command-equivalence audit](task-command-equivalence.md).

- `path-step-2.log`: one public CLI proof passed. PATH selects a wrapper for each
  step; after its first successful operation adds a synthetic future migration,
  the old driver refuses settlement and replay, preserving the selected effect.
  This is simulated schema advancement, not two released lf versions.
- `common-name.log`: shared launch surfaces/provider prompt proof passed.
  `checkout-context-2.log`: three public checkout tests passed, including landed
  Task context and branch identity independent of upstream. Comment refresh can
  fail without erasing confirmed Task context; the first run exposed this and
  the common reader now retains that context with a warning.
- `common-command-proof-4.log`: disposable Linux public CLI proof passed using
  real lf and scripted Linear/Codex/tmux. Managed Task step, direct skill and
  ordinary saved Flow carry the same name and Task seed; independent commands
  preserve the managed review. Revoked credentials in a Flow step mark the
  account missing in SQLite and public auth status. Existing driver-death,
  attached-steer, exact-completion consumption, Chapter rotation and second-Home
  sync-only adoption assertions also passed. Hash inventory and results are in
  `.lf/tmp/cut-i/common-command-proof-4-source.json` and its results directory.
- Earlier public attempts remain counterexamples: attempt 1 sampled before the
  selected native start; attempt 2 sampled before review input publication.
  The fixture now waits for those specific durable events. Attempt 3 found a
  production error-loss bug: Codex failed-turn completion discarded its inline
  error, leaving the account connected. The shared harness now forwards that
  error to existing account invalidation/failover; no Task classifier was added.
- `common-boundary.log`: two moved checkout-boundary tests passed.
  `common-account-preflight.log`: account preflight refusal passed without
  creating a registry. The common account selector enforces the execution
  boundary on each attempt, including inherited account routes and retries.

These are focused branch proofs. Configured account failover/control, repeated
managed decisions, typed blocked/keyed continuation, both-dead explicit retry,
release-before-publication failure and integrated materialized coverage remain
open where not already covered by identical retained bytes. No promotion,
installed-data access, publication or Task completion occurred in this slice.

Final static checks for this common-command slice: `cargo fmt`,
`cargo clippy --all-targets -- -D warnings` (`common-command-clippy-final.log`)
and `git diff --check` passed. Review caught the two snapshot races and the
lost inline provider error above; the public proof was rerun after each repair.

## Structured blocked decisions · 2026-09-30

Jack Heart selected a required reason on the structured blocked result and
same-conversation continuation after the keyed Ask. The command and its separate
agent-issued authority path are deleted.

- `blocked-public-final.log` passed through public lf commands and a real Codex
  engine against a credential-free local Responses fixture. Two blocked turns
  open two Asks; interruption and `flow resume` recover the first Ask; public
  ready/complete returns feedback to three turns in the same AgentSession and
  native thread. Four completions, including the initial work, consume once.
  Final binary SHA-256: `1a4480b309c52a9d2e7c95da9615ff38e2ffce0be907a22572e4ca303a2dfb3f`.
  Results live in `.lf/tmp/cut-i/blocked-public-final-results/results.json`.
- The first public proof failed after driver restart: projecting Blocked into
  the navigation cursor bypassed its Ask. Only Advance/Iterate now project
  there. Blocked remains selected until exact answered feedback is consumed.
  The final key uses only the immutable capture sequence, preserving identity
  through the still-pending child-pass migration.
- `blocked-regressions.log`: five tests passed for output validation, captured
  route restrictions, same-pass blocked consumption, keyed Ask recovery and
  malformed-output correction. Store proof rejects missing completion,
  unanswered feedback and stale reconsumption, and retains the conversation.

The public fixture stubs terminal transport and supplies synthetic feedback;
it proves neither a real interactive Ask nor configured provider acceptance.
A subsequent managed-status wording edit changes “Run” to “conversation” only.
No materialized full matrix, publication, installation or Task completion follows.

Final slice checks passed: formatting, all-target Clippy with warnings denied
(`blocked-clippy-final.log`), Ruff on both touched public fixtures, and diff
whitespace validation. The Chapter fixture change in this slice is formatting only.

## Explicit Task declaration and checkout precedence · 2026-09-30

Jack Heart selected command `--as` → Task checkout → ancestor's explicit `--as`.
`LF_AS` carries only that declaration; the Task execution shortcut sets it to its
selected Task. Provider, tmux and SSH forwarding preserve it. Parent Session,
claim, manifest and `LF_TASK_ORIGIN` no longer assign Work. Exec causal ancestry
is unchanged. The audit table is in [Task command equivalence](task-command-equivalence.md).

- `attribution-proof-5.log`: five focused public proofs passed. X's scripted
  provider invokes real lf in Y's checkout; the child belongs to Y and retains
  X's provider Exec as parent. Both provider environments retain declaration X.
  Other cases cover outside-checkout declaration, explicit override, unbound
  caller ancestry, Ask ownership and installation/PR restrictions.
- `attribution-regressions.log`: 44 of 52 affected cases passed; eight failed.
  Seven reached installed lf via the fixture PATH after the earlier PATH change.
  The fixture now pins its real candidate. The remaining case used headless
  OpenCode inside a confined Task checkout, which the common provider boundary
  rejects. This is not managed OpenCode acceptance.
- `attribution-regression-remainder.log`: six of the exact eight failed cases
  pass after PATH repair. The two remaining fixtures are being corrected: one
  now counts interactive conversations introduced by its adaptation; the other
  fails before Task Flow capture with a child exit. Their latest fixture edits
  are not yet behaviorally verified. Do not count either as a pass.
- `declared_agent_can_start_another_tasks_flow` is authored as an ignored public
  Linux proof: X's provider invokes `lf task run Y`; real driver/mechanical child
  must name Y while retaining the causal chain to X. The disposable attempt
  produced no test output, and a separate `docker info` timed out after ten
  seconds. Forced removal also timed out after fifteen seconds. Cleanup of
  container `06f84fc3f5efbc9e55b0ba861e88779068821fa4bb3d8ee486349046859bda35`
  remains unconfirmed. Only the two verified local fixture-client processes were
  stopped; shared Docker was not restarted. No passing shortcut proof follows.

Review removed stale claims that absent registry files prove no historical Tasks
exist. Without a registry or explicit declaration, ordinary commands remain
unbound; ancestry cannot reconstruct missing ownership. A replaced provider can
issue a new unbound command with its original causal parent, but gains no old
Session/Flow settlement authority. Missing registry with an explicit declaration
and present incompatible registries still refuse mutation.

Jack's operator steer explicitly requested checkpoint/publication before these
remaining proofs to preserve work and obtain hosted Rust/Swift feedback. This
supersedes the earlier pre-publication full-matrix boundary for this partial
checkpoint only. Publication establishes no acceptance, installation, merge or
Task completion; full integrated materialized coverage remains required.

At this checkpoint, `cargo fmt`, all-target Clippy with warnings denied
(`attribution-clippy-final.log`), Ruff on the changed provider fixture, and
`git diff --check` pass. The fixture's final formatting changes do not replace
the two outstanding behavioral reruns.

## Post-checkpoint CI repair · 2026-09-30

Publication `80232a3f2` includes attribution checkpoint `a000e8d68` and the three
preceding commits. Jack's operator read reports hosted Swift passing, Rust
stopping on the obsolete Blocked decoding assertion with 1,613 tests unrun,
and the disposable Task operation unable to find lf on PATH. The retained logs
are `.lf/tmp/cut-i/ci-latest-*.log`; that Rust attempt is incomplete evidence.

The decision test now includes Blocked among canonical values. Step discovery
honors the lock's leading PATH directory, ordinary PATH, selected installation,
then the driver executable. It records the actual executable in the existing
Exec command, without another source-of-truth field or lock mechanism.
`ci-repair-focused.log` passes the canonical-decision and public PATH/schema-gap
proofs, including the operation Exec's command path.

The row/reader regression passes in `attribution-two-remaining.log`. Its other
failure established that attributed Task Flows derive their cwd from the Task,
so overriding the caller cwd could not make a headless OpenCode fixture support
confinement. The fixture now uses scripted Claude and a synthetic private account
route. `task-claude-retry.log` passes failure, explicit retry in the same Session,
review completion, exactly-once progression and independent managed-Flow identity.
The first Claude attempt failed its version probe; the fixture now handles that
probe without consuming its deliberate provider failure. Neither provider fixture
proves configured account acceptance.

Jack requires the complete local Rust suite with no fail-fast before another
publication. The materialized current-tree run is
`ci-repair-materialized-matrix.log`, with source hashes and exact command in
`ci-repair-matrix-source.json`. Results remain pending until that run ends.
The resource preflight passed at 37.9 GiB free against the 32 GiB reserve.
Docker still failed a subsequent ten-second probe; the public Task shortcut and
installed fallback proof remain unexecuted locally.

The first complete post-CI matrix (`ci-repair-materialized-matrix.log`) ran all
2,023 selected tests with `--no-fail-fast`: 2,006 passed, 17 failed, 16 skipped.
Its copied source lacked Git metadata and therefore compiled as release; three
failures expected development behavior. Five account fixtures inherited the
runner's database override. Remaining failures exposed duplicate Codex error
notification, removed parent-env expectations, deleted-Task Ask declaration,
PATH candidate discovery, missing managed fixture accounts, automatic checkout
attribution and newly rendered participant names. These observations are kept;
this run is not a green result. Repairs use the existing owners and fixture
isolation, with explicit development provenance on the next materialized copy.

The Docker probe still timed out at ten seconds. The new executable fallback
matrix and existing Task-Y shortcut proof are now selected by
`scripts/test_task_installation.py`; both remain authored, not locally executed.
The retained container's removal remains unconfirmed. No host service restart,
installation promotion or branch access to installed data occurred.

The first focused repair build stopped at a missing `Path` import in the landing
fixture; the import is fixed. Its follow-up launcher initially named a nonexistent
receipt. Correcting that harness path starts the same materialized snapshot. The
first matrix's log and source copy remain, but the initial receipt filename was
overwritten by the focused-copy setup; do not reuse that receipt for the first run.
The final receipt has its own filename and records the actual command and provenance.

`ci-repair-final-matrix.log` now passes the complete materialized Rust matrix:
**2,024 passed, 17 skipped, none unrun**, `--no-fail-fast`, 778.309 seconds.
`ci-repair-final-source.json` records the development provenance, canonicalized
snapshot and exact command. All 478 Rust/fixture/golden hashes matched the
checkout after the run; only the migration working note differed before this
result was recorded. This supersedes the two unverified attribution regression
reruns, while preserving the earlier failures above. The managed failure/retry/
review fixture uses scripted Claude and a synthetic private account; it is not
configured-provider acceptance.

Review found two consequential repairs: inline Codex failure reporting must
avoid repeating an already emitted error, and relative PATH entries must resolve
against the step's cwd. Both are fixed. Child executable evidence stays in the
existing Exec command; no second selection ledger or Task failure classifier is
introduced. Publication remains a checkpoint. Child-pass migration, released
populated import, configured acceptance and the other remaining-work obligations
stay open.

Final static checks pass: all-target Clippy with `-D warnings`
(`ci-repair-clippy.log`), `cargo fmt --all -- --check`, changed Python Ruff and
`git diff --check`. No new Swift code changed; Jack Heart's reported hosted
Swift pass at `80232a3f2` remains the Swift evidence for this checkpoint.

### Task-shortcut fixture correction · 2026-09-30

Jack Heart's CI reading for `36cbb3d4b` reports full Rust and Swift passes.
The retained `ci-latest-task-installation.log` shows the executable fallback
matrix passing before `declared_agent_can_start_another_tasks_flow` failed:
its Task selected OpenCode, which cannot enforce the checkout boundary.
The remaining installation proofs did not run after that failure.

The corrected fixture explicitly selects Claude for Task Y. Y's saved Flow is
mechanical and starts no provider; X retains interactive scripted OpenCode to
issue `lf task run Y`. The existing assertions still require Y's completed Flow,
its explicit declaration and X's AgentSession in the causal parent tree. No
production code or confinement policy changes. Review confirmed that the
mechanical boundary skips provider-account preflight, so no credentials or
unused provider script are added.

The focused public command is
`uv run python scripts/test_task_installation.py --test declared_agent_can_start_another_tasks_flow`.
Local execution stopped at the ten-second Docker probe, before creating a
container (`task-shortcut-fixture-docker.log`). The repair is authored pending
hosted execution; the prior Rust/Swift passes are not proof of this correction.
Formatting, diff checks and all-target Clippy with `-D warnings` pass
(`task-shortcut-fixture-clippy.log`), including compilation of the repaired test.

### Task command slice review · 2026-09-30

[Review matrix and next action](review-task-command-slice.md) covers
`36cbb3d4b..b341ed3c5`. Docker recovered: the corrected public cross-Task proof
passed in disposable Linux, and its container was removed on exit
(`review-slice-task-shortcut.log`). Three fresh public CLI attribution tests
passed (`review-slice-public-attribution.log`). Both logs are under
`.lf/tmp/cut-i/`. The fixture's prior execution gap is closed; the rest of the
installation suite was not rerun. These are scripted provider/terminal proofs,
not configured acceptance. No production edit was needed during review; the
one-FlowSession migration and the other remaining-work obligations stay open.

## One FlowSession through loop passes · 2026-09-30

Jack Heart selected item 2 only, followed by commit/publication and stop.
The shared driver, Task reader and discovery now use the original FlowSession
through every pass. Retry retains its position; Iterate advances the cursor and
return counters without creating a row or transferring a claim. The Flow parent
field/filter, managed-child view, child lookup and root-lock indirection are gone.
The wire fixture and affected CLI documentation reflect the removal.

The forward `fold_flow_passes` draft preserves original root/child SQL rows in
`import_evidence`, moves AgentSession membership and Flow events to the root,
and copies only the deepest current pass's selection/cursor/review/claim.
Completed siblings contribute history and never overwrite the active position.
Review found that historical import payloads can themselves name removed children:
the archive keeps their exact payload bytes and resolves its foreign key to the
surviving owner. No capture, native receipt, event sequence or payload is rewritten.
Existing migration checksums are untouched.

Focused evidence under `.lf/tmp/cut-i/`:

- `fold-focused.log`: seven source checks pass. Same-pass failure/retry,
  Iterate twice, overlapping counters, stale-result refusal, blocked feedback,
  conversation reuse and populated migration.
- `fold-public.log`: five checks pass. Public taskless two-return Flow records
  one completed FlowSession and three history positions; completed resume starts
  no more agents. Public Task-attributed failure/retry/review and saved-Flow
  paging/detail/wire proofs also pass. Providers are scripted, stores isolated.
- `fold-canonical.log`: the same seven focused checks pass after materialization
  in a disposable exact-source copy. `fold-canonical-source.json` records its
  input hashes and command. The populated nested-pass case retains selected
  completion and mechanical history, pending review identity/title/feedback,
  exact original archive payloads, Task pointer/Started and valid foreign keys.
- `fold-clippy.log`: all-target Clippy with warnings denied passes. Formatting,
  diff whitespace, migration history and architecture coverage pass. The initial
  check found one leftover CLI parent-field reference; it was removed before
  every behavioral proof. Generated architecture HTML is current; rendering
  required the website project environment after the root environment lacked
  `fasthtml`.

Measured against `cbf01f5ab`: Rust production **+28/−200 (net −172)**;
forward SQL migration **+112/−0**; total production **+140/−200 (net −60)**.
Counts exclude integration and inline test code, DTO fixtures, docs, generated
files and supervisor scratch trimming. Inline production is the pre-test portion
of each changed Rust module, retaining durable.rs's public model definitions.
The supervisor's pre-existing scratch trim is preserved separately at `1fb9e32f9`.

No full local suite ran. These are local source/materialized and scripted CLI
proofs, not hosted CI, configured-provider/Desktop acceptance, real-Home conversion,
installation, merge or Task completion. The remaining implementation items and
full-design obligations are unchanged.

Compression after `a66f42a0a` removes the former recursive checkpoint helper,
repeated movement calculation and full Flow clone; pass-test names now describe
positions. The first proof attempt stopped below the 32 GiB reserve; after Jack
Heart's supervisor freed space, preflight passed at 44.7 GiB. All 14 focused checks
pass (`fold-compress-focused.log`, `fold-compress-boundaries.log`), including
Task claim/progress fencing and public Task/taskless retry, loop and review paths
with scripted providers. All-target Clippy with warnings denied
(`fold-compress-clippy.log`), formatting and diff checks pass. No full suite ran.

## Caller-token removal and rebase repairs · 2026-09-30

Jack Heart requested the hosted regressions and remaining-work item 3 together,
then commit, publish and stop. Baseline: `7f34e9087e`.

- Main's ordinary typed skill command is `skill -- NAME`; the review argv proof
  now includes that reserved-name escape and still proves saved instructions run
  after source removal. Mechanical fixture definitions now use `cmd:` throughout
  both requested files, including the disposable installation cases.
- The shared command's automatic checkpoint used only explicit binding flags.
  It now consults the existing checkout/inherited-declaration binding reader too,
  preserving the Task's managed Flow, HEAD and another contributor's edits.
  That fix exposed a fixture depending on an incidental earlier commit: its
  unbound launch committed the skill only on another branch. The fixture now
  commits its shared skill before branch changes.
- The forward draft archives non-null old Exec caller tokens in immutable
  `import_evidence`, then drops the column and unique index. Existing selected
  Flow payloads and all other Exec evidence remain unchanged. No applied SQL is
  edited. The unused Rust selection token, caller fields, Swift/DTO mirrors and
  obsolete parent environment handling are deleted. Tests use native start-event
  references rather than synthetic tool-command Execs.
- Review retained `agent_parent`: matching provider generation/origin resolves
  the current conversation driver; stale engines retain the proven original
  parent. Causation grants neither Work attribution nor settlement authority.

Initial compilation caught the reduced environment-array length; it was corrected
before behavioral execution. `caller-files-2.log` ran all 56 selected tests without
fail-fast: 51 passed, five failed, four skipped. Four failures were additional
`op:` fixtures, and one was the branch-local skill fixture above. Their fixes
are included in the final run. No failure is counted as a passing proof.

The final file run (`caller-files-final.log`) completed without fail-fast:
55 passed, one fixture setup failed, four skipped. Its commit helper required
explicit staging; that setup is fixed and the exact case is rerun separately.
The 15 selected source unit checks (`caller-units-final.log`) passed 13 and
found two fixture mismatches: missing Flow identity in migration setup and one
remaining three-variable environment expectation. Both are corrected for the
materialized proof. Compilation also exposed three assertions that exercised
only the deleted fixture caller lookup; those assertions are removed. Actual
selected-turn, failed-turn and conflicting-output behavior remains tested.

Focused completion: `caller-public-final.log` passes all three selected proofs:
repaired checkout fixture, Rust Exec DTO and actual Codex parent handoff. Together
with `caller-files-final.log`, all 56 runnable cases in the requested two files
pass (four disposable-installation cases remain skipped), without rerunning the
whole repository. The Session file contributes 33 passes. Real Codex 0.159.0 uses
credential-free local Responses and isolated Homes; its command parents follow
the replacement driver, while a replaced provider retains its historical parent.
This is not configured-account acceptance. `caller-swift.log` passes all 18 Swift
DTO tests.

Production measured against `7f34e9087e`: Rust/Swift code **+20/−34 (net −14)**;
forward SQL **+11/−0**; combined **+31/−34 (net −3)**. The per-file receipt is
`caller-lines.log`. Counts exclude inline/integration tests, DTO fixtures,
scratch and generated files. The removed fixture lookup and three assertions
are test deletion, not production reduction.

The first materialized check (`caller-canonical.log`) passed three of four
cases and found the fixture's missing selected-event reference. After adding
the existing Session/native-start evidence, `caller-migration-final.log` passes
the populated source upgrade. The first three materialized passes retain exact
native-result settlement, Exec discovery and the repaired environment fixture.
`caller-canonical-final.log` passes the populated upgrade on the final
materialized schema. `caller-canonical-final-source.json` records the exact
source hashes, command, development provenance and successful exit; Rust,
Swift, Cargo and fixture inputs still match the checkout. All-target Clippy
with warnings denied passes on final bytes (`caller-clippy-final.log`), as do
formatting, diff whitespace, migration history and architecture coverage.
No full local suite was run. No configured-provider, rendered Desktop,
installed-Home conversion, merge or Task completion is claimed.
The disposable fallback attempt (`caller-install.log`) stopped at Docker's
ten-second probe, before creating a container. It did not reach the corrected
assertions. No host service restart or installed-Home access was attempted.

Compression after `974f1dfd5` shares the test-only native-turn event lookup,
removes redundant actor aliases, and corrects the obsolete caller-Exec ownership
comment. All 13 focused Flow/Task checks pass (`caller-compress-focused.log`);
all-target Clippy with warnings denied, formatting and diff checks pass. Runtime
and migration bytes are unchanged; no full suite or configured acceptance ran.

## Landing fixture repair · 2026-09-30

Jack Heart requested the hosted failure at `f07dcb6b8` be repaired before the
separate naming commit. The repair-proof Flow now uses main's `cmd:` syntax.
`naming-land.log` passes all 24 `land_tests` with `--no-fail-fast`, including
`lf_pr_land_waits_for_authoritative_merged_observation`. These use real local lf
processes with simulated GitHub/provider effects and isolated data; no hosted or
configured acceptance follows. All-target Clippy with warnings denied
(`naming-land-clippy.log`), formatting and diff checks pass. Production delta is
zero; the one-line fixture change preserves the authoritative-merge assertions.

## Execution and capture naming · 2026-09-30

Jack Heart requested item 4 as its own commit after the landing repair
`f02565d8d`. [Before/after names and the two unselected table proposals](naming.md)
record the complete boundary. Process entry APIs use Exec; captures and recorder
history use Session names; Flow definition compilation uses `compile_*`.
`TaskFlowView` needs definition provenance for its `from …` breadcrumb, so that
data remains as `sources` in Rust, Swift and current DTO fixtures. Historical
saved graphs still encode `flow_parents`; manifest Rust fields now describe
artifact/caller/request facts while serde preserves `run_id`, `parent_run_id`
and `launch`. No SQL, environment encoding, old selector or execution owner is
replaced by an alias, parallel writer or new lifecycle.

Focused evidence under `.lf/tmp/cut-i/`:

- `naming-focused.log`: **31 passed**, no fail-fast. Covers saved codec equality,
  source compilation and graph projection, Rust DTOs, reserved publication and
  conflicting-input refusal, replay, completed keyed Ask import, populated
  historical captures/SQL-only members, exact taskless completion and the
  authoritative-merge landing scenario. One nextest leaky-handle diagnostic on
  the saved-skill codec test is retained; this is not a clean-handle claim.
- `naming-session.log`: **7 passed** after the final Session helper and diagnostic
  renames. Public interactive Session and Ask lifecycles, malformed-caller
  admission refusal, replay, manifest encoding, prepared capture and completed
  keyed Ask preservation. `naming-admission.log` separately retains the earlier
  one-case admission pass. These selections overlap; do not sum them as unique
  scenarios. Providers and terminal transport are scripted, data isolated.
- `naming-swift.log`: **19 passed**, DTO fixtures plus Flow catalog decoding with
  a direct `sources` assertion. The app compiles; no rendered Desktop or real
  terminal continuity acceptance follows.
- `naming-final-static.log`: final all-target Clippy with warnings denied passes.
  Formatting and diff checks pass. Architecture coverage passes in
  `naming-architecture.log`; `naming-docs.log` records documentation sync,
  architecture HTML regeneration and its freshness check. No migration changed.

Review caught stale manifest-field references during compilation and one
remaining admission assertion using the old wording; these were corrected before
the final focused proofs. It also caught cached website Markdown rendering old
model prose; sync now precedes regeneration. The retained provenance consumer
prevents deletion; the saved codec prevents a rename from rewriting immutable
capture bytes. Public graph wire changes have no defaults or compatibility alias.
No behavioral or authority change was selected.

Rename-aware production comparison against `f02565d8d`: **+938/−890, net +48
Rust/Swift lines** (`naming-lines.log`). Excludes inline `cfg(test)` items, test
files/modules, DTO fixtures, docs, generated output and scratch; no SQL is added.
Moved modules are compared to their original paths rather than counted as deletion.
Longer names reflow signatures and literals; the figure is not a reduction claim.

No full local suite ran. This checkpoint does not establish configured-provider,
rendered Desktop, released-populated public/canonical import, installed-Home
conversion, merge or Task completion. Item 5 remains the next implementation cut.

Compression after `a6150f52e` removes repeated type/module qualification and corrects
capture-owner prose: **−37 production lines**, leaving the naming cut **+11 net**.
No compatibility decoder, wrapper or alias was added by that cut; six serde key
mappings and the pre-existing saved-skill codec retain stored formats. An import
collision with the public membership projection was reverted. All ten focused
capture/replay/codec/history checks pass (`naming-compress-focused.log`); final
all-target Clippy, formatting and diff checks pass (`naming-compress-clippy-verified.log`).
No full suite or configured acceptance was run.

## Deep current-state compression · 2026-09-30

Jack Heart removed historical-import and intermediate-draft obligations. Three direct migrations replace 38 drafts; old Run/import readers, archives, codecs and tests are deleted. Final affected Rust run: **363 passed**, no fail-fast (`deep-affected-green.log`); Swift DTO: **18 passed**. Build, fmt, all-target Clippy with `-D warnings`, architecture, migration history and generated-doc checks pass. Earlier failed fixtures and their repairs remain in `deep-*` logs; no full repository gate is claimed.
Fresh and copied-database rehearsals pass; original operating columns match, 59 resumable Sessions and 20 Flows decode, and foreign keys hold. Live filesystem changes prevent an atomic-snapshot claim; actual conversion still requires quiescence. Installed data was never opened for writing. [Inventory and cutover boundary](compress-history.md). Net **7,884** lines removed, including tests/docs and new migrations, relative to `c7fcd31ec`.
