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
another executable writer. All bounded contributors have returned their artifacts
to main; no contributor/tool handle remains active at this observation.

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

Published **4ced9467fde48b88b232c346af054ff9e3ad8f58**, based on
**7d1158dac2f0a7db420efadc76dcce20da05e2c1**. GitHub PR1296 and Task publication
agree; auto-merge is absent. Latest integration includes upstream Xcode cache reuse.
Main now converts saved native continuation; unclassified SQL history still
prevents dropping `runs`. Preserve the full remaining-work checklist.

Latest terminal CI inspected is **36541353761** on the preceding `6f927ea5f`:
Rust job 109317277279 has 1,834 passes, one OpenCode taskless-decision failure,
15 skips and 134 unrun tests. The error requires the selected native turn's
original caller. Log `.lf/tmp/cut-i/ci-6f927ea5f-rust.log`; no full-green claim.
Hosted interruption proof passed on earlier 9887f8c84; exact controlled Linux
failure/repair receipts remain in the main handoff. New-head CI needs inspection.

Final/events now read retained Session history, including landing conclusions.
Five focused cases passed; the summary fixture initially failed with LEAK because
of an invalid input ID, then passed alone after correction. Retain both receipts.
Summary SQL excludes transcript payloads before Rust hydration, preserving usage
and unknown evidence; dense latency/write cost remains unmeasured. JSONL writers
still serve unconverted native/active consumers. This is not complete Run removal.
Named mechanical operations retain SQL observations in Flow history, with unknown
observed start/Exec. A non-null skill label alone never establishes an agent.

The failed-turn child race is reproduced with real Codex and scripted Responses:
an old child decides after automatic retry begins, and the Flow wrongly completes.
The caller repair rejects that child but also the legitimate retry because the
failed native thread retains its tool environment. Unsubscribe/resume experiments
failed; the successful-thread control refreshed it. Jack has not answered the
pending decision-interface choice in questions.md. No provider fork, tool proxy,
shared-engine kill, new attempt object or retry-contract change is selected.
Independent work continues; missing-decision and valid-retry proofs stay required.

## Returned research and proof boundaries

| Artifact / contribution | Reusable finding and outstanding limit |
| --- | --- |
| [Run removal](research-final-run-removal.md) | Twelve source hashes verified; separate obsolete CRUD from historical SQL and live lifecycle readers. Main removed CRUD; table deletion still owed. |
| [Native retry options](research-native-retry-options.md) | Nine local source/schema hashes and upstream reload predicate verified. Four tradeoffs, no selected transport or passing valid retry. |
| [Indexed discovery](research-indexed-discovery.md) | SQL-only fixture is not CLI latency proof. Narrowing review lookup drops nullable historical membership; preserve discovery. Desktop still requests unlimited inventory. |
| [Exec admission](research-exec-admission.md) | Thirty-nine hashes checked; only main handoff drifted. Early paths and two hard exits escape logging; malformed-caller agent admission needs a public probe. No code/test contribution. |
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
canonical populated preservation, final DTOs/indexed discovery and code-complete
review remain explicit obligations. A test assertion with a leak is not clean
settlement. No partial import count permits deleting unresolved history.

## Comparable production measurement

`f7f12a757` against 359c9a6c3: **+15,334 /−29,666 = net −14,332**.
Rust/Swift +14,217/−29,604; Python/shell +48/−62; SQL +1,069/−0.
Receipt `.lf/tmp/execution-model/status-counts-f7f12a757.json` reproduces the
preceding 1d86cd4f3 receipt. It predates the final/events cut and latest rebase.
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
