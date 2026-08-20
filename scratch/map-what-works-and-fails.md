# Map the Current `lf` Agent Experience

## Problem

The 2026-08-19 work repaired three concrete thorns: bare terminal control now
preserves launch options, shared reads default to the current repository, and
the Mac app uses the `lf` bundled with that exact app build. Those mechanisms
shipped. They did not make the whole agent experience trustworthy.

LOO-234 maps the installed v0.12.10 experience as observed on 2026-08-20. It
separates source-level capability from reproduced behavior and preserves the
User's explicit **Red** judgment of the Product Wave Discord agent: LLM-cliche
voice, process narration, weak continuity with User momentum, and reactive
focus. Missing or stale evidence remains unknown; it never rounds up to green.

The evidence standard is:

- **Works:** reproduced through the installed or real configured path on
  2026-08-20, with the executable source revision and observable result.
- **Fails:** reproduced through the real path, or contradicted by a durable
  receipt from real operation.
- **Stale:** the newest evidence predates the 2026-08-19 changes or cannot be
  joined to the installed build.
- **Unknown:** no available receipt distinguishes success from failure.

A source diff or isolated test supports a finding. It does not prove the
installed product path by itself.

## Reviewed intent

The User's two Steers set a narrow boundary: map the seven named surfaces,
preserve the explicit Discord Red judgment, identify the gap that matters most,
and explore an internal-tool or dashboard shape. This Task does not select or
implement that follow-up product surface. The current findings and priority
decision below are evidence-backed; `lf evidence`, its DTO, and Podium rendering
remain one candidate shape for later work.

## The demo

From this Task worktree, run the reproduction commands below with installed
`lf` v0.12.10. The result is deliberately mixed: terminal launch selection and
repository scoping reproduce; `status` fails on LOO-225; `roadmap` preserves the
Wave while marking Projects unavailable; Ask attention exposes two stranded
claims; and Product Discord history quantifies the Red experience rather than
hiding it.

A credible follow-up demo is one evidence board, backed by a shared `lf` JSON
projection and then rendered by the Podium, where the User-authored Red
assessment sits beside fresh measurements and every row opens its Run, Ask,
Task event, Discord message, command receipt, or commit. Whether that projection
is a new `lf evidence` command or an extension of an existing truth surface is
still open.

## Approach

Treat the current map as an evidence report, not a health score. Each finding
has three independent parts:

1. **Authored assessment** — who judged the experience, when, and from which
   durable input. The Product Discord verdict remains Red because the User said
   Red; an improving counter does not silently overwrite that judgment.
2. **Measured observation** — a value, window, freshness boundary, and explicit
   `pass | fail | collecting | unknown` result. No sample and stale samples are
   unknown, not zero and not healthy.
3. **Evidence references** — typed links to the raw record. A summary without a
   reference is itself an Auditability failure.

The highest-leverage product gap is **parent-to-child control authority**.
Direct Task Runs execute when authority is established, but Wave/Project Turns
cannot reliably pass their Run/Turn basis into orchestration subprocesses. That
blocks delegation, blocks the repair Task meant to fix delegation, and can even
block a User Ask. Repair the existing LOO-237 Work through an independently
authorized Run before retrying LOO-233. The Discord publication contract is
next: internal skill/process narration belongs in the Run record, while Discord
should receive one boundary-level response shaped around the User's momentum.

For the evidence-board candidate, `lf` should own one typed, repository-scoped
snapshot and let the Podium decode it. Reuse the proven `PodiumReading` states
(`loading`, `available`, `unavailable(lastGood, reason)`) and the lifecycle
scorecard's `pass`, `fail`, `collecting`, and `unknown` measurement semantics.
Do not create a Swift-owned truth model, a second transcript, or a synthetic
Wave score.

If pursued, the smallest useful snapshot would contain:

```text
EvidenceSnapshot
  generated_at, repo, wave
  producer { version, source_revision, binary_identity, home_id }
  assessments[] {
    subject, verdict, reason, author, authored_at, source_ref
  }
  projects[] {
    project_id, slug
    krs[] {
      fingerprint, text, linear_holds
      assessment?
      measurements[] {
        id, value, unit, window, eligible, measured,
        verdict, reason, observed_at, stale_after
      }
      evidence_refs[]
    }
  }
  failures[] { subject, reason, evidence_refs[] }
```

`PmKr` currently has only `text` and `holds`; it has no durable KR id. Fingerprint
`project_id + exact KR text`. Editing a KR creates a new fingerprint and resets
its evidence to unknown. Never carry a prior green assessment across changed
proof language by fuzzy matching.

An `EvidenceRef` names one existing authority: `lf_exec`, `run`, `invocation`,
`turn`, `ask`, `task_event`, `steer`, `discord_message`, `linear_item`, or
`git_commit`. The first version is a read projection over existing records. It
does not introduce another event store.

## Current evidence map

Observed against release source revision
`011122c2d68d995cfe65dd6fd907d88f0ce28cef` on 2026-08-20. Both the installed
CLI and `/Applications/Loopflow.app`'s bundled helper report that revision via
`lf doctor --json`.

| Surface | Finding | Evidence | Boundary / near-miss |
|---|---|---|---|
| Terminal-control launch options | **Works for the reported option-preservation path.** Installed `lf -m claude` with an isolated fake provider logged `lf loopflow -m claude`, selected the Claude managed account, and exited 0. | Installed CLI hash `f708ec2e…`, source revision `011122c2d…`; shipped commit `cdb1f4a3d` (#1200). | This proves parsing, assembly, model selection, and provider launch. A real interactive vendor handoff was not exercised and remains unknown. |
| Repository-scoped reads | **Works.** `lf ls --json` from this linked worktree and canonical main returned the same five Wave ids; `--all` returned 24 machine-wide Waves. | Reproduced from `/Users/jack/src/loopflow.map-what-works-and-fails` and `/Users/jack/src/loopflow`; shipped commit `3feade2ef` (#1201). | A fixture containing one repo would not prove filtering. The live shared ledger did. |
| Ask handoff | **Fails end to end; durable creation/presentation is partial success.** The current repo queue exposed four User Asks: two active and two still `claimed` after their answer Runs ended at `2026-08-20T09:40:03Z`, both with `handback: unknown`. No fresh receipt proves answer -> requester resume. | `ask_d6cbb120…`, `ask_06461c65…`; shipped durable Ask commit `156623e47` (#1175). | Ask rows, tmux attach argv, and active presentation prove durable surfaces exist. They do not prove handoff completion. |
| Status and roadmap truth | **Fails overall; roadmap degrades honestly.** `lf status product --json` exits 1 and emits no snapshot because completed Task LOO-225 violates the reviewer-copy invariant. `lf activity --task LOO-234` also fails while reading LOO-225. `lf roadmap --json` returns the Product Wave but marks `projects.state: unavailable` with the exact decoder reason. | LOO-225 (`c2c91762…`); error `invalid Task: merge request requires reviewer-facing PR copy`; roadmap observation at `2026-08-20T19:02:14Z`. | A top-level Wave row is useful, but it cannot answer “what is this wave doing?” while all Projects are unavailable. Roadmap's reason is truthful; status's total loss is not useful. |
| Child-control authority | **Fails on parent launch paths.** Auditability Project Run `run_742ce634…` reached its Turn but could not reserve LOO-233 because no durable Turn Basis reached `lf task run`. LOO-237 allocated Task/worktree/PR identity and initially failed for missing Run execution context. A later independently started Task invocation reached the provider but failed `codex thread not started`; that still does not prove Project-to-Task control. | LOO-233 remains unreserved; LOO-237 Task `task_21b193…`, PR `pr_123680…`, Task event `64750`; Product memory commits `c11c3ed38` and `219747585`. | LOO-235 and this LOO-234 Run prove direct Task execution works with established authority. A live parent containment, Task allocation, or direct Task Run is not proof that parent child-control recovered. |
| Installed-build provenance | **Works at the diagnostic/build boundary; missing from ordinary receipts.** The global CLI and signed app helper have different content hashes but both report release provenance, published migration authority, and exact source revision `011122c2d…`. The signed app is v0.12.10 and contains executable `lf`/`lfd` helpers. | CLI hash `f708ec2e…`; app-helper hash `8858e48c…`; `/Applications/Loopflow.app` Developer ID signature; shipped commit `afc4010e8` (#1205). | Version `0.12.10` alone is insufficient. `doctor` can join the binaries to source, but `status`, `roadmap`, Ask, and chat snapshots do not carry producer provenance per receipt. |
| Product Wave agent in Discord `#product` | **Fails — explicit User Red judgment.** The raw transcript corroborates the shape: after the last User message on `2026-07-21T18:07:09Z`, 56/56 messages were assistant-authored; 43 began with “I’m using …”. In one recent 25-minute window, 13 assistant messages averaged 1,091 characters, 12 opened with process narration, and the agent repeated clarify/pursue/mutate cycles around whichever receipt changed last. | User judgment source: Task Steer `epoch_4603a3408cc24ed89827509f21233c08:1`. Last User message: Discord `1529187996908392560`. Example current boundary: Discord `1540072126940319795`. | Transport, dedupe, and linked Linear ids can work while the conversation is still Red. Do not translate delivery success or autonomous activity volume into conversational quality. |

## Discord diagnosis

The User's Red assessment is preserved as authored judgment, not inferred from
message counts. The quantitative record explains why it is credible:

- **LLM-cliche voice:** repeated stock transitions (“key distinction”, “finish
  line”, “the evidence confirms”) make different situations sound identical.
- **Process narration:** 43 of 56 assistant messages after the last User message
  announce the internal skill being used. The user sees the orchestration loop
  instead of its consequence.
- **Weak continuity with User momentum:** the last User request asked for a terse
  “changed / demo / next risk” report. No later User input exists in the epoch,
  yet 56 agent messages followed.
- **Reactive focus:** recent output cycles through clarify, pursue, and mutate as
  LOO-233, LOO-237, and LOO-235 receipts change. It accurately notices new
  facts, but the visible conversation is driven by scheduler churn rather than
  a stable user-level thread.

The raw Run record should retain every phase. Discord should publish only a new
user-relevant boundary: decision, completed work, changed blocker, Ask, or
requested report. Silence is a valid result for no-op clarify/pursue/mutate
passes.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Did the 2026-08-19 launch fix reach the installed release? | Yes. The installed binary reproduced `lf -m claude` through provider selection and exit 0 under isolation. | Mark the narrow claim Works; keep full interactive handoff Unknown. |
| Does repository scoping work against a genuinely multi-repo ledger and linked worktree? | Yes: 5 current-repo Waves versus 24 machine-wide, with identical ids from main and this worktree. | No follow-up design needed for the shipped filter. Preserve `--all` as explicit operator scope. |
| Can one invalid Task be contained by the truth surfaces? | Roadmap contains it as a Project-unavailable reason; status and activity still abort. | Evidence projection must isolate failure per subject and retain the rest of the snapshot. LOO-233 remains the focused repair after authority recovery. |
| Does durable Ask imply successful handoff? | No. Two claimed Asks point to ended Runs and unknown handback. | Measure create -> claim -> present -> settle -> requester-resume as one funnel. Do not count rows or presentations as success. |
| Is child-control failure a general executor failure? | No. Direct Task Runs execute when authority is established; parent-launched control lacks Turn/Run context. | Scope the repair to parent-to-child authority propagation. Reuse LOO-237; do not create another Task. |
| Can installed commands be tied to source? | Yes through `doctor`; no in each ordinary snapshot. | Put producer source revision and binary identity on the evidence snapshot and every captured probe batch. |
| Can current KRs be joined by a stable provider id? | No. `PmKr` is `{text, holds}` and Swift also uses text as id. | Fingerprint exact text under Project id; a text edit starts unknown rather than inheriting evidence. |
| Does a green aggregate score fit the Auditability contract? | No. Current evidence mixes narrow Works, hard Fails, and Unknown boundaries. | Show authored assessment and measurements side by side. Never average them into one score. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Keep this as a manually updated Markdown scorecard | Fast and readable; easy to annotate with judgment. | It becomes stale immediately, cannot enforce freshness, and cannot prove zero orphaned claims. Keep this document as the design/evidence seed, not the product surface. |
| Add a Mac-only dashboard fed by several subprocess calls | Strong visual scan and can reuse current Podium components. | It makes the app the only place the agent cannot inspect, risks another Swift truth model, and cannot repair CLI/Discord auditability. The typed `lf` snapshot must come first. |
| Derive one automatic Wave health score from counters | Compact and sortable. | It would smooth unavailable evidence and the User's explicit Red judgment into false precision. This repeats the deleted Wave-score mistake. |
| Store assessments and metrics in Linear Project prose | Keeps planning near KRs. | Linear is the authored plan, not the live evidence ledger; freshness, local Run links, and per-command provenance would drift or require network reads. |

## Decisions and follow-up assumptions

- **This Task ends at the map.** It records the current evidence, selects the
  highest-leverage gap, and explores a product shape without opening an
  implementation slice.
- **Repair authority before retrying work.** LOO-237 is the existing repair
  identity. Start it only through an independently authorized Run; do not ask the
  broken parent path to repair itself and do not reserve LOO-233 first.
- **Judgment and measurement stay separate.** A human Red can coexist with a
  passing transport counter. Only a new authored judgment changes the Red.
- **Unknown is first-class.** Missing samples, stale windows, unavailable readers,
  and unproven end-to-end paths remain visibly unknown.
- **Evidence remains inspectable.** Every non-unknown claim has a typed reference;
  a `pm doctor`-class check rejects a claim whose target record is absent or
  whose freshness window cannot be computed.

The following are candidate choices, not User-confirmed implementation scope:

- **Shared CLI contract before visualization.** A typed `lf` projection is the
  API; the Podium renders it. `lf evidence --json` is a working command name,
  not a settled one.
- **One bad record damages one row.** The evidence snapshot returns the healthy
  Wave plus a typed LOO-225 failure; it does not abort the full response. This
  consumes LOO-233's containment repair rather than rebuilding it.
- **Do not persist a second transcript.** Discord source URLs and durable Turn ids
  are references; the evidence board does not copy conversation history.
- **Silence no-op Wave phases.** Clarify/pursue/mutate details remain in raw Runs.
  Discord receives only user-relevant boundaries.

## What remains uncertain

- A real interactive vendor handoff was not exercised; only the installed
  option-preservation and provider-selection path is proven.
- No fresh receipt proves Ask answer -> requester resume, or parent-launched
  child control after authority repair.
- The shared evidence projection's command boundary is not selected. Reusing
  `status`, `roadmap`, or `activity` may be truer than adding `lf evidence`.
- The proposed Discord publication boundary has not been validated by a fresh
  User conversation. Until a new authored assessment says otherwise, the
  current experience remains Red.

## Wild success

The User opens Product and immediately sees **Red — conversation breaks
momentum**, the exact authored Steer behind that judgment, fresh measures such as
“56 assistant messages since last User input” and “43 process-narrated,” and a
link to the raw Discord messages. Beside it, repository scoping is green, full
vendor handoff is unknown, and child control is red with the exact failed Task
event. When LOO-237 repairs authority, its measurement changes without erasing
the User's Discord judgment. The same snapshot appears in CLI, Podium, and agent
context.

## Wild failure

The dashboard becomes another reassuring summary. It paints missing data green,
lets old evidence survive edited KRs, copies Discord into a second store, and
reduces 13 heterogeneous rows to “72% healthy.” Agents then quote the score in
Discord with more process narration while `status` still exits 1. That surface
would deepen the Auditability failure and should be removed.

## Scope

- In scope: the seven named agent surfaces; evidence freshness and installed
  provenance; the explicit Product Discord Red judgment; the highest-leverage
  next gap; and a concrete typed internal-tool/dashboard shape.
- Explored but not committed here: one repository-scoped `lf` DTO,
  per-subject failure isolation, KR fingerprints, authored assessments,
  quantitative measurements, typed references, and Podium rendering of the same
  fixture.
- Out of scope: implementing the evidence tool in this design-only Task; repairing
  LOO-225, LOO-233, or LOO-237 here; changing Linear KRs; building another
  transcript; or treating Discord transport health as conversation quality.

## Done when

This design-only Task is done when review can trace every map claim to the named
command, source revision, durable id, Discord URL, or commit; reproduce the mixed
results without external writes; and make one explicit priority decision from
the evidence. This reviewed document does that without turning the exploratory
dashboard shape into implementation scope.

If the candidate evidence slice is selected later, its provisional proof is:

1. One shared `lf` JSON projection emits producer provenance, authored
   assessments, fresh measurements, typed references, and explicit failures.
2. Injecting an invalid Task fixture leaves all unrelated Waves/Projects/KRs
   readable and marks only the affected subject unavailable with its reason.
3. Expired, missing, or insufficient measurement windows render `unknown` or
   `collecting`, never pass.
4. The Product Discord judgment remains Red until a new sourced assessment
   supersedes Steer `epoch_4603a3408cc24ed89827509f21233c08:1`.
5. Every non-unknown assessment and measurement resolves at least one evidence
   reference; the orphan check reports zero.
6. CLI and Podium decode the same fixture and present the same verdict, reason,
   freshness, and drill-down target.

These conditions advance Auditability's “what is this wave doing?”, visible
reason, and zero-orphaned-claims KRs. They also expose—without claiming to have
fixed—the end-to-end drill-down KR and the Loopflow API spawn-chain KR.

## Measure

Capture before and after for the same Product Wave and source revision:

- `status`/`activity` usable subjects divided by requested subjects;
- Ask handoffs completing create -> settle -> requester resume, N/N over seven
  days;
- evidence claims with live references divided by all non-unknown claims;
- measurements fresh inside their declared window divided by all measurements;
- Discord assistant messages since the last User message, process-narrated
  messages, and boundary-level messages;
- parent-launched child controls that obtain a Task Run and first provider Turn,
  N/N over seven days.

The baseline is not green: status and scoped activity abort on LOO-225; 2/4
current User Asks are stranded claimed with unknown handback; parent-to-child
control fails; and Product Discord has 56 assistant messages after the last User
input, 43 process-narrated. Keep each denominator and missing sample visible.

## Reproduction commands

All commands are read-only except the terminal-control probe, which uses a fresh
temporary `LF_HOME` and a fake provider executable.

```bash
lf doctor --json                    # exact installed source revision
lf ls --json                        # 5 repository Waves
lf ls --all --json                  # 24 machine-wide Waves
lf status product --json            # exits 1 on invalid LOO-225
lf roadmap --json                   # Product survives; Projects unavailable + reason
lf activity --task LOO-234 --json   # fails while reading LOO-225 PRs
lf ask list --user --json           # 2 active, 2 stranded claimed
lf task status LOO-237 --json       # current child-control repair receipt
lf chat --history -w product --limit 200 --json
lf runs --wave product --json
```
