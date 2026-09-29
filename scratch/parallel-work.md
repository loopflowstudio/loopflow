# LOO-298 control and scratch index

2026-09-28 · Jack Heart requested autonomous progress to a **code-complete
concept review**, Codex only. Regular rebase/publication and useful bounded
parallel work are authorized. Landing, auto-merge, promotion, real-Home migration
and branch-binary access to the installed Home are not.

## Read order and ownership

1. [Accepted contract](data-model-one-table-per.md): objects and invariants.
2. [Main handoff](parallel-execution.md): implementation and next proof.
3. [Remaining work](remaining-work.md): complete unfinished scope.
4. [Import preservation](import-preservation.md): historical counterexamples.
5. [Chapters](chapters.md): Project/default Flow contract and proof gaps.
6. [Evidence](evidence.md): recorded results and limits.
7. [Open assumptions](questions.md): unresolved policy.

Main owns executable edits, builds/tests, cleanup, Git and
`parallel-execution.md`. Supervisor owns other scratch and isolated nonbuilding
inspection/proofs. Builds remain serialized. Never checkpoint another active
contribution or create a competing implementation.

Current managed worker, verified through Task status and OS-live `lf ps`:

- Saved feature invocation `1f9ba70e-f0e9-41d4-9722-948d7bc4ce8c`.
- Implement iteration 10, Run `run_4f129189186948279f5cc9416d7897e6`.
- Worker Exec `eb97ad88-3bf1-40d1-8077-260473473744`, PID87749;
  Codex PID18549 at observation. Refresh before recovery; silence is not death.
- Preserve the captured implement → compress → review-slice → concept-review →
  loop-decide → human-demo order. Intermediate concept review is not the goal's
  code-complete review. Supervisor chooses no Flow edge.

The bounded Run-removal researcher returned exit0; tool handle `71697` is closed.
Main now owns `scratch/research-final-run-removal.md` for checkpointing. Supervisor
verified all twelve source hashes and checked importer, history projection, CRUD
and trigger findings. Verified handback `023cdc33-045c-4aab-a915-e11447ed2b21`.
The audit separates obsolete Run CRUD, historical SQL preservation, immutable
input references and still-live file-backed lifecycle readers. No executable
edit or behavioral test ran. Log `.lf/tmp/cut-i/run-removal-research.log`.

The bounded native-retry researcher returned exit0; handle `4457` is closed.
Main now owns `scratch/research-native-retry-options.md` (94 lines). Nine local
source/schema hashes match; supervisor independently matched the upstream
thread-processor hash and inspected its reload predicate. Handback
`ede31e16-4e35-42ee-a84f-7b4a1f2df3d7` retains the failed-thread and same-thread
observer limits, four concrete repair tradeoffs and the paired acceptance proof.
No new transport, provider fork, engine kill, upstream message or retry-policy
change is selected. Log `.lf/tmp/cut-i/native-retry-options-research.log`.

The Chapter CLI contributor returned
`tests/e2e/chapter_rotation.py` to main with exit0; tool handle `2564` is closed.
Handback `.lf/tmp/cut-i/chapter-cli-handback.md` retains the original source hash;
main subsequently formatted the fixture and added the nonempty PR assertion.
It authors public second-Home sync and actual Project-default Task launch proof,
requiring a scripted provider's completion consumed by the real worker. No
execution state is seeded. Ruff passes (`chapter-static-final.log`); Linux fixture
execution remains outstanding. Main owns the file and serialized execution in a
disposable account/container. CI's task-installation job does not run this fixture,
and CI36527817576 provides no reusable CLI artifact. Its green installation result
does not establish Chapter/default-Flow proof. Original contributor log:
`.lf/tmp/cut-i/chapter-cli-contributor.log`.

The Desktop contribution returned exit0;
tool handle `50100` is closed. Verified handback
`8ff3937a-f2a9-4675-af30-657c13cf687a` returns all six released Swift paths to main.
Five changed: projection, breadcrumb bar, PodiumModel and two WorkspaceNavigation
test files; SessionsView stayed unchanged. Known bound ancestry survives absent
planning and remains outside orphan grouping. Production +70/-11, net +59;
supervisor verified source hashes and scoped whitespace. The contributor ran no
builds/tests. Main subsequently passed all 25 WorkspaceNavigation tests and the
extended mounted `namedSessionDrillDownRetainsTerminal` proof on those same bytes.
The native proof uses fixture readings/test PTYs and timer rendering; it is scoped
pane/draft retention evidence, not configured Desktop acceptance.
Exact hashes/commands/limits: `.lf/tmp/cut-i/desktop-ancestry-handback.md`;
log `.lf/tmp/cut-i/desktop-ancestry-contributor.log`. No DTO/schema or new inventory.

The earlier Task initialization contributor
completed with exit0 and returned its file in verified comment
`9464adc8-3045-4b71-82a0-22b9eaaa1a8d`; main owns it again. Review publication
and readiness use AgentSession input APIs. The contribution exposed a missing
caller relation and a manifest read in Task status. Main now stores
`caller_input_id` with the immutable input reference; `task_waiting_unblock`
reads SQL. Supervisor inspected the completed source conversion and retained
stale/current Ask, completion-without-navigation and wrong-boundary assertions.
The exact stale/current Ask CLI integration now passes in
`agent-admission-cli-recovery.log`; the contributor ran no tests.
Starting dirty bytes: `.lf/tmp/cut-i/task-initialization-before.rs.txt`; log:
`.lf/tmp/cut-i/task-initialization-contributor.log`; handle `10626` is closed.

Earlier bounded contributions are finished and returned to main:

- Eight Chapter builtin skills: scoped source/whitespace review, no live planning
  mutation or behavioral proof; preserved in the local checkpoint.
- Status fixtures: main's corrected materialized suite passed all eleven tests.
- `ops/chapter_tests.rs`: exact non-null Started and sync-only second-store
  adoption assertions returned in comment `5d29a5f6-92eb-4bf6-ac50-09b15a6bf590`.
  Main's focused pass now passes all 15 Chapter tests, including exact Started
  preservation and sync-only second-store adoption. Returned
  SHA-256 `e24360cfca5f0ec6207eed39a7b9048482e1e56c3633a8678bc654d18eea446c`.
  Log `chapter-proof-contributor.log`; tool handle 8813 is closed. Public CLI
  sync and actual default-Flow launch are still separate obligations.

## Captured installed control

Use `.lf/tmp/cut-i/control-checkpoint.py`. It pins both executable and Home:

- Binary `/Users/jack/.lf/bin/lf-f5ef8d640340e9f8b9e36d17d84de83e14e905305c43e959fd00c49a322a527f`.
- `LF_BIN` and `LF_CONTROL_BIN` both select that binary.
- `LF_HOME` and `LF_CONTROL_HOME` both select
  `/Users/jack/.lf-dev/installed/local-afee63d734c7482cb94d1071af26d9ea`.
- `LF_DB_PATH` and `LF_CONTROL_DB_PATH` both select its `loopflow.db`.

Bare lf selects a different Home without this Task; absence there does not prove
worker death. Do not repair machine selection/auth as a prerequisite. Source
proof clears inherited LF/LOOPFLOW authority and uses private Homes; installation
proof uses the existing disposable OS harness.

Refresh `session list --json` at each user turn and after Session mutation.
Control Session `run_9c16dbbe2b04440db8469e9b4964912c` has human title `loopflow`;
preserve it. Questions for Jack stay here. Earlier completed concept-review or
unblock Sessions are not fresh approval. Task comments beginning Supervisor are
supervisor direction, never new decisions attributed to Jack.

## Current work and delivery

Current published head is **29d5c5229e19c3179a33e17a34ca1248faf8a522**, based on
**d4b283a873804c18ef1894d49bb690d67f4d59f5** (#1343, Swift compilation reuse).
GitHub, Task publication and Git merge-base agree; auto-merge is absent.
CI **36535480097** finished with 1,831 Rust passes, the same two cutover failures,
15 skips and 134 unrun of 1,967; log `ci-29d5c5229-rust.log`. This publishes the
populated migration repair and retained evidence. Rebase initially refused before any sequencer while
supervisor notes were dirty; after their released checkpoint, rebase succeeded
with the same base. `lf commit --no-add --push` published the exact checkpoint
without the then-active research artifact. Main is repairing the cutover tests;
OpenCode's missing native selection is an integration question, not assumed to
be only stale fixture wiring. All remaining-work obligations stay open.

The source Session suite subsequently finishes all twenty cases with fifteen
passes and five failures (`session-cutover-provider-repair.log`). Direct provider
PID/start recording repairs the scripted OpenCode continuation/review path;
selected native decision integration stays red. Four remaining history/fixture
cases then yield two passes and two inherited-source failures
(`session-cutover-history-fixtures.log`). Task/Wave agree; original source needs
proper evidence rather than mutable current-Session fallback. Verified direction
`233e4769-5fb7-4ff7-ad91-8545c1ae7f33`. These are local intermediate results, not
a green Session suite or full CI. Main retains their implementation and proof.

Previous published head **24ea61517cc5cbcfc4cf29452364fe6efcd37123**, same base:
CI **36533494844** finished with failure: 1,829 Rust passes, two failures,
15 skips and 134 unrun tests. The Task Flow fixture still queries obsolete Run
rows; the taskless decision stand-in lacks native caller authority. Verified
direction `3e204ff4-cad2-4f9a-8df4-446478da0d29` preserves history/review/ancestry
assertions and the production navigation check. Log `ci-24ea61517-rust.log`.
Scratch-clear also fails on active notes; all other test/lint/install jobs pass,
merge-proof is skipped and the aggregate tests-result fails. These failures do
not establish the separate valid-native-retry repair. SQL-only history and the authorized
checkout fixture are published; final all-target Clippy passed in
`historical-sql-static-final.log` (15.30s). Main continues populated historical
migration preservation before remaining Run deletion. Full scope stays open.

Historical CI, publication, rebase and repair chronology is preserved in
`29d5c5229:scratch/parallel-work.md` and the exact pre-curation archive below.
Detailed implementation receipts live in [main handoff](parallel-execution.md)
and [evidence](evidence.md); the full finish line remains in
[remaining work](remaining-work.md) and [import preservation](import-preservation.md).
Do not restore old Run writes to green a stale fixture or count moved code as deletion.

Retain these proof boundaries through subsequent conversion:

- Conversation readers preserve each input's ancestry/provider/timing and missingness,
  including pre-bind usage and recent continuation of old conversations. Four
  focused reader checks pass in `conversation-readers-final.log`; native ordinary
  retry/public usage additionally passes in `conversation-native-usage-2.log`.
  Native-only driver-loss usage, final DTOs and complete owner removal remain owed.
- Import preserves captured headless/review distinctions, completed keyed answers,
  replaced inputs and SQL-only members in the recorded scoped proofs. The
  populated source/canonical frontier and missing-title limit belong in
  `import-preservation.md`; no partial count permits dropping unmapped evidence.
- Git-free auth records actual Exec outcomes; subdirectory commands keep actual cwd
  with optional journal files at checkout root. The scorecard child survived lf
  interruption in the strengthened reproduction; the existing process-group guard
  repairs that boundary. `exec-interruption-child-fixed.log` has two passes and no
  LEAK. This proves neither all provider termination nor Task-wide settlement.
- The shared harness records the caller for checkout decision fixtures; those
  focused passes do not repair the real native retry or establish OpenCode parity.
  Preserve upstream ownership filtering, uncapped caller lookup, activity windows,
  resource/cache semantics and the prior conflict-resolution checks.
- Earlier broad materialized diagnostics recorded 1,925 passes, 33 failures and
  15 skips. Later focused checks describe mixed snapshots, not a full green gate.
  Mapping: `.lf/tmp/cut-i/supervisor-matrix-dispositions.json`. Hosted full CI,
  final materialized preservation and code-complete review remain required.
- The removed intermediate concept review survives at
  `aa43c6829c:scratch/concept-review.md`; it was not the goal review. Configured
  Desktop/provider, Linux Chapter/default-Flow, and real-Home acceptance retain
  their separately recorded gaps. An assertion pass with a process leak is not
  clean settlement. Full remaining scope is unchanged by this curation.

The valid native decision retry still fails. The paired late-child test reproduces
the stale decision and confirms rejection after the local attribution repair,
but the failed Codex thread retains its old tool environment and also rejects the
legitimate retry. The bounded unsubscribe/resume experiments were negative;
the successful-thread control refreshed the environment. Keep one original-turn
attribution check; no provider fork, tool proxy, shared-engine kill or retry-policy
change is selected. See [tradeoff](native-turn-retry-tradeoff.md). Substantial
expansion needs a concrete smallest proposal with cost and preservation proof.
Independent owner conversion continues; this does not require another Ask Session.

The quoted-tool-output classifier repair is local and tested, not deployed to
captured control. Preserve command events and actual provider outcomes; old log
text is not control authority. Exact historical operational recoveries and the
retained regression are in evidence.md and `unblock-quoted-output.md`.

## Comparable production measurement

Published **24ea61517 against d4b283a87**:
**+15,501 / −29,637 = net −14,136**.
Rust/Swift +14,451/−29,575; Python/shell +48/−62; SQL +1,002/−0.
Receipt `.lf/tmp/execution-model/status-counts-24ea61517.json` reproduces the
preceding da19 measurement first. Current migration edits are excluded.

Earlier whole-branch measurements and exact receipts are retained in the archive
and committed note above. Changed bases prevent comparing totals as per-cut
changes. Method: `measure-published-cd4ab9d813.py`, corrected production prefixes,
no rename detection, tests/docs excluded, trailing Rust test modules removed while
known trailing production stays counted. The latest receipt reproduces its
predecessor before measuring. Current edits are excluded. No completion-time or
final-size estimate is established; main owns fresh resource preflight/cleanup.

## Archives

Consolidation preserved **62 files / 676,767 bytes** at
`.lf/tmp/scratch-consolidation-20260928/scratch/`, with sibling SHA manifest.
It includes uncommitted designs and non-Markdown fixtures/patches. Eleven earlier
LOO-291 originals remain at `.lf/tmp/context-archive-7e2101b41/scratch/from-loo291/`.
Published historical originals are also in checkpoint `7e2101b41`. Preserve local
archives until useful evidence has a durable owner before delivery clearing.

This control-note reduction preserves its exact prior 16,478 bytes at
`.lf/tmp/scratch-curation-20260928-control/parallel-work-5b29beffb67e.md`, SHA-256
`5b29beffb67e99d34128f14ea8213637324ecc6cb540b373fc19f105e9f3c4bb`.
Detailed CI chronology already belongs in evidence.md. Archived old Session/Run,
Chapter-table and warn-and-proceed models do not override the current contract.
No failed result, acceptance obligation or unresolved attribution is erased by
curation. Main's `parallel-execution.md` remains untouched.

The 2026-09-29 control-note curation preserved the exact preceding 30,715
bytes at `.lf/tmp/scratch-curation-20260929-control/parallel-work-a87f6837dd9f.md`, SHA-256
`a87f6837dd9f5e0888dd8d9c11e59a1de01ccbb0382dc19680c8d904535265ea`. It removes repeated chronology and older measurements from active
context, not evidence or unfinished requirements. Main's handoff and all other
artifacts remain unchanged.
