# LOO-298 control and scratch index

Consolidated 2026-09-28 at Jack Heart's request. Goal: autonomous progress to a
**code-complete concept review**, Codex only. Regular rebase/publication is
authorized; landing, promotion and branch-binary access to installed Home are not.

## Read order and ownership

1. [Accepted contract](data-model-one-table-per.md): objects, invariants, finish line.
2. [Main handoff](parallel-execution.md): live implementation and next proof.
3. [Remaining work](remaining-work.md): full unfinished scope and incident dispositions.
4. [Import preservation](import-preservation.md): migration counterexamples.
5. [Chapters](chapters.md): current Project/default Flow contract and proof limits.
6. [Evidence](evidence.md): current failures and retained scoped passes.
7. [Open assumptions](questions.md): unresolved policy, not historical debate.

Main owns all executable files, builds, cleanup, Git and `parallel-execution.md`.
Supervisor owns the other scratch documents and isolated nonbuilding proofs.
Prior contributors are finished; their executable ownership returned to main.
Useful parallelism remains authorized through lf, with disjoint ownership and
serialized integration/builds. Do not create a competing implementation or bypass
the saved Flow's implement → compress → review-slice → concept-review →
loop-decide → human-demo order. Curation chooses no edge.

## Captured installed control

Binary: `/Users/jack/.lf/bin/lf-f5ef8d640340e9f8b9e36d17d84de83e14e905305c43e959fd00c49a322a527f`.
Set **both `LF_BIN` and `LF_CONTROL_BIN`** to it for managed launches.
Set `LF_HOME` and `LF_CONTROL_HOME` to
`/Users/jack/.lf-dev/installed/local-afee63d734c7482cb94d1071af26d9ea`, and
`LF_DB_PATH`/`LF_CONTROL_DB_PATH` to that Home's `loopflow.db`.
Bare lf selects a different Home without this Task; absence there is not worker death.
Do not repair machine selection/auth as a prerequisite. Source proof uses private
Homes with inherited authority removed; captured installed control is not promotion.

At each user turn and after Session mutation refresh `session list --json`.
Current control Session `run_9c16dbbe2b04440db8469e9b4964912c` has human title
`loopflow`; preserve it. Ask Jack here, not via a new Ask Session.

## Published state and comparable size

Published head `26a0270af604ac14abbd20df9a020336bee85a0a`, base
`d9632d833c8216b00cde117656d12172eda00c2c`,
[PR1296](https://github.com/loopflowstudio/loopflow/pull/1296).
GitHub and Task publication heads matched on 2026-09-28; Task base and Git
merge-base match the integrated main. Auto-merge is absent. Hosted run
36507832289 completed with one Rust failure (882 passed, 1,050 unrun), scratch-clear
and aggregate failure; all other jobs passed. The exact metric-reader failure
is retained in evidence.md.
Jack's docs/README-first direction is the next substantive step, before further
owner conversion; main acknowledged it and has begun reading those pages.
Refresh shared state before delivery actions.

Current measured checkpoint, **26a0270af against d9632d833**, has production
**+14,240 / −29,502 = net −15,262**: Rust/Swift+13,447/−29,446;
Python/shell+47/−56; SQL+746/−0. Method excludes tests/docs, disables rename
accounting, strips trailing Rust tests but retains known trailing production code.
It reproduced 7e210's previous count exactly before measuring the new head.
Receipt `.lf/tmp/execution-model/status-counts-26a0270af.json`; reproduction
`measure-published-26a0270af.py` in that directory. No working edits are counted.
The prior 7e210/a2b59 receipt remains: +13,855/−29,473, net −15,618. Changed bases
and moved code must not be presented as additional deletion.
Earlier forecast 20–40 active hours/3–5 working days,
final net −20k to −28k was low-confidence, not a current measurement or commitment.

Last disk sample 69.7GiB free / 64GiB floor; active builds 53.3GiB, inactive 0;
uv cache 12.95GiB. Only supported worktree-prune candidate was 74 MiB dogfood.
Supervisor removed no worktree/cache; main is cleanup owner. Refresh preflight;
these numbers confer no authority to remove another active contribution.

## Archive and coverage

Before full consolidation, all **62 files / 676,767 bytes** (54 Markdown files /
604,510 bytes) were copied byte-for-byte to:

`.lf/tmp/scratch-consolidation-20260928/scratch/`

Sibling `manifest.json` records every path, byte count, SHA-256 and source HEAD.
It includes dirty designs/ledger and all eight non-Markdown patches/fixtures.
Archive is ignored local evidence, not another automatic prompt source. Preserve
it until useful evidence is retained durably before delivery scratch clearing.
Published originals are also available at
[checkpoint 7e2101b41](https://github.com/loopflowstudio/loopflow/tree/7e2101b411ecb95c1c7785070c36a9ed90b6df4b/scratch).
The full archive is necessary for uncommitted updates; Git alone would miss them.

The earlier 11 LOO-291 originals were already replaced by links before this pass.
Their original 225,509 bytes remain in
`.lf/tmp/context-archive-7e2101b41/scratch/from-loo291/`, with SHA manifest,
and were byte-compared to published 7e210. This archive retains the later stubs too.

| Retired material | Active destination |
| --- | --- |
| Cuts 1–G, H, I and Exec ownership logs; historical questions/reviews | Accepted contract, evidence and main handoff; exact chronology in archive |
| Import review/four-origin fixture | Import preservation; archived fixture remains reproduction input, not a current passing test |
| H7 contributions/review/shared patches | Chapters + evidence; integrated patches must not be replayed |
| Native research/approval/handoff reviews | Remaining authority/continuity work + evidence; exact native probes archived/referenced |
| Discovery/performance/initialization/rebase reports | Remaining discovery/Desktop work + evidence; prior scale/timing limits retained |
| Publication/stacking/incidents and patches | Remaining incident dispositions + evidence; source-only reports superseded by named integrated proofs |
| Resource/release/website CI reports | Evidence and current CI; detailed repaired failures in archive |
| LOO291 UX/design/demo + old model/demo/control notes | Current contract/remaining Desktop proof; historical designs are not fresh approvals |

No requirement, failed result or deployment gap becomes accepted by shortening
these notes. Old Session↔Run models, warn-and-proceed launches, relaxed ancestry,
Chapter tables and completed foundation-only claims are superseded by the current
contract. Attribution unresolved in old records stays unresolved.
