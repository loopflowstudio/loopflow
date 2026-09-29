# LOO-298 control and scratch index

2026-09-29 · Jack Heart requested autonomous progress to a **code-complete
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

Main owns executable edits, builds/tests, cleanup, Git and its handoff. Supervisor
owns this index, other assigned scratch and isolated nonbuilding inspection/proofs.
Builds stay serialized. Never checkpoint another active contribution or introduce
another executable writer. The wire/Desktop researcher finished with tool64028
exit zero; its 106-line artifact and 73-file receipt were reviewed. The bounded
Swift-fixture researcher (tool77919) also exited zero. Its artifact
`scratch/research-active-swift-fixture.md` is released with an unapplied patch;
all twelve source/log hashes match and five diff hunk counts were checked.
It made no source edits or behavioral proof. Main integrated the proposal; the
focused realCLI transport test passed in 10.370s and hosted Swift/UI passed on
003fa792f. The Exec entry/exit contributor (tool8752) completed exit zero.
Its [unapplied proposal](proposal-exec-entry.md) is released: artifact SHA-256
`4b89f5af436fc28f5527498372200b40ff980deee6e94b5002072899ae28f46d`;
all 25 diff hunks and the embedded digest were checked. Only run_record.rs
changed among source fingerprints, matching main's documented native-usage work.
Main owns source integration and builds. A new bounded numeric Flow wire
contributor is active (tool41910, Exec `c79afcab-2b09-4098-bfdf-7e74da8eeccd`,
PID56307). Its sole tracked output is `scratch/proposal-numeric-flow-wire.md`:
an unapplied Rust/Swift/fixture patch using the existing wire research. No shared
source edits, builds/tests, Git or PM mutations. Main continues Exec/usage work;
coordinate rebase with source fingerprint handback. Verified scope comment
`59189fa4-5373-42bc-a04a-ff8ae02371bd` preserves the boundary.

Supervisor review removes the proposal's extra `process_is_recorded` check:
it reads journal events, while driver/provider FKs already require the Exec row.
Keep the real-CLI/library distinction and selective Exec-write-failure proof.
Early parser/startup/install/screenshot persistence remains unfinished; temporary
zero-row assertions are not permanent product policy or missing user approval.
A safe noninitializing writer is the remaining technical boundary. Proposal
formatting/parsing is not compilation or behavioral evidence; apply hunks only.

Main's `exec-entry-proof.log` now passes 12 focused CLI tests (19.758s, after
24.36s compilation): exact SSH/release exits, interruption, inspection without
Started, and provider refusal when Exec insertion fails or caller context is
malformed. The two parser cases retain the explicitly unfinished early-store gap;
they are not proof of complete process logging. `linux-interrupt-final.log` is
older evidence: its receipt names head1d86cd4f3 and differs in the entry/exit files.
The new wrapper still needs its Linux interruption proof on matching bytes.


## Active control

- Saved feature invocation `1f9ba70e-f0e9-41d4-9722-948d7bc4ce8c`, implement
  iteration 10, Run `run_4f129189186948279f5cc9416d7897e6`.
- Worker Exec `eb97ad88-3bf1-40d1-8077-260473473744`, PID87749; Codex PID18549.
  Refresh Task status and OS-live `lf ps` before recovery; silence is not death.
- Preserve captured implement → compress → review-slice → concept-review →
  loop-decide → human-demo order. Supervisor chooses no Flow edge. The earlier
  review at `aa43c6829c:scratch/concept-review.md` was not code-complete review.
- Control Session `run_9c16dbbe2b04440db8469e9b4964912c` has human title
  `loopflow`; preserve it. Refresh Session list at each turn and after mutation.
  Questions for Jack stay here, never another Ask. Completed reviews are not
  fresh approval; Supervisor comments are not new decisions attributed to Jack.

Use `.lf/tmp/cut-i/control-checkpoint.py`, which pins both executable and Home:

- Binary `/Users/jack/.lf/bin/lf-f5ef8d640340e9f8b9e36d17d84de83e14e905305c43e959fd00c49a322a527f`.
- `LF_BIN` / `LF_CONTROL_BIN` select that binary.
- `LF_HOME` / `LF_CONTROL_HOME` select
  `/Users/jack/.lf-dev/installed/local-afee63d734c7482cb94d1071af26d9ea`.
- `LF_DB_PATH` / `LF_CONTROL_DB_PATH` select its `loopflow.db`.

Bare lf selects another Home; absence there does not prove death. Source proofs
clear inherited LF/LOOPFLOW authority and pin private Homes/executables. Installation
proof uses a disposable OS account/container. Do not repair auth/placement merely
to make an inspection pass. Task-specific contributions use `--task LOO-298`;
`--wave infrastructure` relocated one researcher to the repo root despite cwd.

## Delivery and next work

Published **858444d9175248f0bc5987fc2f3b1a1786365a42**, based on
**4439aedd9afb6deae779f646b667ab1a1e90d43b**. GitHub and Task publication agree;
auto-merge is absent. In CI36551670877, Swift109351070547, UI109351070444 and
installation passed. Rust109351070825 failed the same OpenCode decision case:
1,841 passed, one failed, 15 skipped, 134 unrun (199.184s). Log
`.lf/tmp/cut-i/ci-858444d91-rust.log`; both child decisions lacked original-turn
authority. The workflow is now terminal failure; other executed jobs passed
except scratch-clear and its aggregate tests-result. Prior ad8cb3a7b
CI36549325005 is terminal: Swift109343328459 and UI passed;
Rust109343328423 failed the retained OpenCode taskless decision case after 1,840
passes, one failure, 15 skips and 134 unrun (194.705s). Both step child commands
refused original-turn authority. Log `ci-ad8cb3a7b-rust.log` under `.lf/tmp/cut-i/`;
this is the same provider integration gap, not another database fixture. Main
has the exact output. Other executed jobs passed except scratch-clear. Table removal is checkpoint dab41cf48.
Comparison from old003fa792f to rebased d5eef693c contains only upstream4439
doctor_tests.rs isolation (PR1349); no extra test repeat is required.

Earlier base was567ac07df. Continuation checkpoint dca1d617d
rebased to 3280c7524 with one reconciled migration file. Upstream prefix hashing
joins this branch's outer initialization transaction; the focused initialization/
append/adoption contention regression passed once (1.021s). This was not a
byte-for-byte upstream transplant.

CI **36548046448** on 003fa792f is terminal. Swift and UI passed; Rust job
**109339098854** stopped after 1,062 passes, one failure, 15 skips and 912 unrun
(81.781s). The deleted-Task fixture restored its database override before the
final native-history assertion, then opened the wrong database. Main moved
restoration after that assertion; its focused local case now passes in 0.940s.
Other executed jobs passed except scratch-clear; no full-green claim follows.
Earlier 3280's publication/Swift admission fixtures are repaired and published.
The preceding 4ced9467f Rust job retains its separate OpenCode decision failure
(1,835 passes / 135 unrun); the current cutoff does not retest it. Hosted
interruption proof passed on earlier 9887f8c84; retain its controlled Linux scope.

Published table-removal cut retains every historical SQL row as immutable input
catalog evidence, with nullable conversation attachment. Unknown inputs still
report as unresolved; no AgentSession or Exec is invented. Ordinary replacement
again requires a fresh input INSERT after supervisor review caught earlier-input
reuse. The six-case public/bind/ancestry/mechanical/Started/deleted-Task proof
passes in `historical-table-public-2.log` (7.618s).

`historical-table-canonical.log` passes both released-position and populated
development upgrades (0.881s, 0.905s). The latter compares every original SQL
column, asserts `runs` absent, retains unknown attachment and history references,
and exercises immutability, ancestry, Started and replay. Supervisor compared all
2,020 entries in `canonical-input-evidence-source.json`: production Rust and the
migration match; only two docs and the Codex fixture assertion differ. All-target
Clippy passes in `historical-table-static.log` (16.11s). The edited Codex fixture
has not supplied fresh native proof. Table removal is a bounded local result;
final input/wire readers, native recovery and the complete checklist remain open.

Final/events now read retained Session history, including landing conclusions.
Five focused cases passed; the summary fixture initially failed with LEAK because
of an invalid input ID, then passed alone after correction. Retain both receipts.
Summary SQL excludes transcript payloads before Rust hydration, preserving usage
and unknown evidence; dense latency/write cost remains unmeasured. JSONL writers
still serve unconverted native/active consumers. This is not complete Run removal.
Named mechanical operations retain SQL observations in Flow history, with unknown
observed start/Exec. A non-null skill label alone never establishes an agent.

Provider thread/account publication and lookup now use retained Session history.
The ordering/import/summary proof passes three cases, and the five focused native
durability/resume checks pass after the reducer change. Original event order and
fresh-versus-imported precedence are retained; no provider-session sidecar writer
remains. Saved continuation, prefix lookup and active metadata now use SQL input
identity after manifest removal; initial publication and exact process receipts
remain. The isolated concurrent-review cleanup trace and nextest passed, without
establishing the earlier LEAK's cause. These fixtures do not establish configured
native recovery. The continuation cut is +142/-210 production Rust lines (net -68).

The failed-turn child race is reproduced with real Codex and scripted Responses:
an old child decides after automatic retry begins, and the Flow wrongly completes.
The caller repair rejects that child but also the legitimate retry because the
failed native thread retains its tool environment. Unsubscribe/resume experiments
failed; the successful-thread control refreshed it. Jack has not answered the
pending decision-interface choice in questions.md. No provider fork, tool proxy,
shared-engine kill, new attempt object or retry-contract change is selected.
Independent work continues; missing-decision and valid-retry proofs stay required.

Native-only usage now records the original immutable input on new native starts;
old unmapped starts stay unknown. Supervisor's source counterexample (20→40→30)
first failed, then passed after every snapshot used the existing cumulative
reducer. `native-usage-decrease-green.log` passes replacement, deduplication,
missing baseline and retained peak in one case (0.406s). The materialized
`native-usage-canonical.log` passes that case and populated development migration
(0.238s, 0.642s). All 2,022 source hashes matched when reviewed. Later edits to
the native CLI fixture remain separate. The strengthened `native-usage-public-final.log`
then passed on candidate `0a4ffa6b…f8`: both lf receivers exited, missed-turn SQL
usage/completion were zero, reconnect retained the original interrupted/130 Exec,
and public usage reported 120/30 once. A native inspector remained connected;
this is real Codex with scripted Responses, not zero-native-client or configured
account proof. Final Clippy passed (17.45s); the later predecessor proof passed
(0.402s). Those later reducer bytes were not part of the canonical snapshot.

Supervisor found a further concrete counterexample after publication:
`.lf/tmp/execution-model/supervisor-native-baseline-gap/{probe.py,results.json}`.
Copied candidate `7b59f6db…23f`, scrubbed authority and a disposable Home; no provider.
Seed sequential same-input turns A=20, B=30, C=10. B has a final recorder receipt
and native start/completion but no native usage; C's native total is60, last10.
Public usage reports **90 tokens, gaps0**, expected60. The baseline search skips B
to A and assigns B's30 to C again. Main received verified comment
`3ef3e767-d002-45ac-aa55-47329f38e0d3`: use immediate-predecessor evidence or retain
a partial suffix/gap. This is a seeded missing-notification case, not an observed
write failure. Main's regression reproduces90 in `native-baseline-gap-red.log`
(0.394s), then passes60 with a gap in `native-baseline-gap-green.log` (0.407s).
The earlier replacement/deduplication/peak case also passes (0.417s). The reader
repair is +12/-15 production lines; no new state. These are focused source proofs;
the previous public120/30 transport receipt still names its earlier candidate.

## Returned research and proof boundaries

| Artifact / contribution | Reusable finding and outstanding limit |
| --- | --- |
| [Run removal](research-final-run-removal.md) | Twelve source hashes verified; separate obsolete CRUD from historical SQL and live lifecycle readers. Main removed CRUD; table removal now has populated canonical proof and is published. |
| [Native retry options](research-native-retry-options.md) | Nine local source/schema hashes and upstream reload predicate verified. Four tradeoffs, no selected transport or passing valid retry. |
| [Indexed discovery](research-indexed-discovery.md) | SQL-only fixture is not CLI latency proof. Narrowing review lookup drops nullable historical membership; preserve discovery. Desktop still requests unlimited inventory. |
| [Exec admission](research-exec-admission.md) | Thirty-nine hashes checked; only main handoff drifted. Early paths and two hard exits escape logging; malformed-caller agent admission needs a public probe. No code/test contribution. |
| [Session wire/Desktop](research-session-wire.md) | Seventy-three files checked: 70 unchanged, three main-owned lifecycle files changed without public declaration changes. Retain original/closing hashes and supervisor drift receipt. Maps Session discovery, numeric graph identity, and typed history consumers; no tests or new product policy. |
| Chapter CLI | `tests/e2e/chapter_rotation.py`, `.lf/tmp/cut-i/chapter-cli-handback.md`: Ruff passed; disposable Linux execution is still owed. CI task-installation is not this proof. Fifteen Chapter unit cases are narrower evidence. |
| Desktop ancestry | `.lf/tmp/cut-i/desktop-ancestry-handback.md`: 25 navigation tests and mounted PTY retention pass on returned bytes. Not configured Desktop, final DTO/headless discovery or performance acceptance. |
| Task initialization / status | Returned and integrated; stale/current Ask CLI and eleven materialized status tests pass within recorded scope. No remaining parallel writer. |

Keep earlier failed materialized matrix (1,925 pass / 33 fail / 15 skip) and its
per-failure dispositions at `.lf/tmp/cut-i/supervisor-matrix-dispositions.json`.
Later focused repairs do not make that snapshot green. Preserve historical
attribution, pre-bind usage, exact worker/Session/Flow fences, activity end windows,
uncapped child lookup, and upstream installation/cache semantics through reduction.
The quoted-output classifier repair is source-tested, not deployed to this captured
control binary. Old tool output cannot become current execution authority.

Configured provider/Desktop, real-Home import, public Chapter/default-Flow execution,
complete historical preservation, final DTOs/indexed discovery and code-complete
review remain explicit obligations. A test assertion with a leak is not clean
settlement. No partial import count permits deleting unresolved history.

## Comparable production measurement

Published `858444d91` against its base `4439aedd9`:
**+15,912 / −29,888 = net −13,976**. Rust/Swift +14,706/−29,826;
Python/shell +48/−62; SQL +1,158/−0. Receipt
`.lf/tmp/execution-model/status-counts-858444d91.json`; excludes working bytes.
Working changes at that read were +211/-83 (net+128), a moving snapshot only.
GitHub main advanced to `6e7189926ede81b3edd05c8d81955c6cbe67a4e2`
(PR1351, native capture concurrency); main owns the next coordinated rebase.
The table-removal cut alone is Rust+56/−42, SQL+81/−0 (net+95), because the old
released/draft migration history remains while forward preservation drops its
runtime table. Earlier receipts retain their own bases and measured snapshots.

Method: corrected production prefixes in `measure-published-cd4ab9d813.py`, no
rename detection, tests/docs excluded. Changed bases prevent treating differences
between whole-branch totals as incremental cuts. No final-size/time estimate proven.

## Preserved history

Earlier consolidation retained 62 files/676,767 bytes under
`.lf/tmp/scratch-consolidation-20260928/scratch/` with its SHA manifest, plus eleven
LOO-291 originals under `.lf/tmp/context-archive-7e2101b41/scratch/from-loo291/`.
Checkpoint 7e2101b41 preserves published originals. Retain private archives until
useful evidence has a durable owner before delivery clears scratch. Historical
models do not override current decisions.

This index's exact prior bytes, including completed-contributor handbacks and
older delivery/count receipts, remain in the archive and committed version below.
Main handoff, evidence, complete scope and design are unchanged by this curation.

- Commit: `4ced9467fde48b88b232c346af054ff9e3ad8f58:scratch/parallel-work.md`.
- Archive: `.lf/tmp/scratch-curation-20260929-control/parallel-work-d7f33ebf4cfe.md` (16,561 bytes).
- SHA-256: `d7f33ebf4cfea55654221bf03f1642f08a36244a61ec04c9d79d874a80b51417`.
