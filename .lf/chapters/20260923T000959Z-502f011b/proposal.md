> Current status: Gate 2 accepted on 2026-09-23; all accepted operations are applied and verified; the chapter is sealed. The dated acceptance and corrections at the end supersede pending language in the preserved proposal below.

# Reconciled chapter proposal — Gate 2 pending

**Keep three Waves, rewrite five existing Projects, and judge seven KRs. Open five Task slots across Loopflow, with one serial implementation slot per Project.** Preserve capacity for Cube, Etude, Kata, and Hootro; the human chooses their priorities. This is the root proposal after all Wave/Project contributions, not an accepted or applied plan.

UTC interval: **2026-09-23 00:09:59 → 2026-10-21 00:09:59** (22 September → 20 October in Los Angeles). Parent Run: `run_502f011b9d1f4596ba62eff0e54a8b0e`. All retained Waves and Projects share this proposed review.

## Decision record and evidence

Gate 1 was explicitly accepted in the live parent instruction. [Accepted direction](accepted-direction.md) remains authoritative. No interview or Ask was opened. Gate 2 has not been accepted.

The imported [manual baseline](../20260922-manual-baseline/review.md) retains 39 judgments: 2 hold, 16 do not hold, 21 unknown. The [frozen starting ledger](frozen-ledger.md) includes five registered Waves, nine current PM Projects, **42 KRs and twelve open Tasks**. List contributes three unreviewed KRs beyond the manual baseline. Five newer Desktop Tasks are included. Nothing was silently inherited or retrospectively marked successful.

Current cache-only PM reads at the end matched every frozen Project and Task, including definitions/directives and cache timestamps. This is cache consistency, not a new provider sync. Engbot has no local PM snapshot or charter; its empty focused status remains an explicit evidence limit.

## Accepted Wave boundaries; proposed portfolios

| Wave | Owns | Proposed Projects |
| --- | --- | --- |
| Infrastructure | Loopflow self-hosting reliability, releases, auth, execution continuity and repository-wide reduction | Reliability; Architecture Minimalism |
| Intelligence | Local context/Run evidence for explanation and diagnosis, with no execution or priority authority | Trace & Context |
| Product | External usefulness, shared user contract, current planning joined to native Sessions | Company Dogfood; Desktop |

List and Engbot are proposed for retirement, preserving history. External Work stays in its own repositories and Waves. Small (1–2), Medium (2–4), Big (2–8) are company review heuristics; they add no runtime limits or Work kinds. Account juggling and prompting evidence support selected workflows, rather than form disconnected bets.

## Exact proposed definitions and KR ledger

All new claims start unproven. The linked proof contracts are mandatory acceptance detail, not optional polish. Historical successes remain attached to their original windows.

### Infrastructure / Reliability

Reuse Project `03c59a52-dbd4-4e11-a6ac-f6d6359c8e08`; rewrite `stability-security` without replacing identity.

Maintainers can keep Loopflow advancing its own work and delivering verified releases without repeatedly repairing its coordination machinery. Scheduled delivery reaches truthful, recoverable outcomes; planning and execution remain usable across credential expiry, process failure, and upgrades. Recovery preserves work, history, and exact control authority.

**R1.** Throughout the chapter, every configured scheduled release opportunity is durably accounted for as on-time, caught up, deferred with a recorded reason and continuation, or failed with an actionable cause. Two consecutive opportunities settle through the configured path as a truthful no-op or complete exact-tag publication; at least one publishes artifacts available to users, all required verification and exact-tag smoke checks pass, and neither settlement requires manual repair. Required scheduled verification failures remain visible and receive an owning repair disposition within one day.

**R2.** Across fourteen consecutive days of real Loopflow self-hosting, every attempted core planning or execution operation reaches observable progress, an intentional declared wait, or one bounded, truthful, actionable failure. Ordinary credential expiry with a usable refresh grant does not interrupt planning; recoverable process or controller failures resume without manual state repair. There are no hidden outages, duplicate advancements, lost durable input, terminal-truth regressions, or false availability claims. Recovery preserves assigned-worktree isolation, caller edits, durable history, and exact process authority.

[Full proof contract and reconciliation](proposals/reliability.md).

### Infrastructure / Architecture Minimalism

Reuse Project `62b73cde-5057-4959-8ad8-fca96b9e80b5`; rewrite `technical-architecture` without replacing identity.

Loopflow stays legible and smaller as it evolves. Public APIs, durable data, process boundaries, and control authority map to intentional product concepts and one owner. Repository-wide reduction removes obsolete paths and duplicated truth; code, CLI, DTOs, user documentation, and the architecture map describe the same supported system. Changes preserve active work, human decisions, and historical evidence.

**A1.** At chapter review, every public, durable, and cross-process concept in the reviewed repository has one intentional owner and truth source, with no unexplained duplicate authority or obsolete active path. The repository is simpler in named, reviewable ways than the chapter baseline, and implementation, CLI, DTOs, user documentation, and architecture map agree at the same revision. Four actual weekly checks judge the same reviewed source locally and in the hosted environment and retain their scope, findings, and coverage limits; reconstructed receipts do not replace missing observations. Every changed execution or placement path passes its recorded preservation and authority counterexamples.

[Full proof contract and reconciliation](architecture-reconciliation.md).

### Intelligence / Trace & Context

Reuse Project `2f8390f7-6614-426c-84a8-fa5291358691`; rewrite `trace` without replacing identity.

Human-selected portfolio work is explainable from durable local evidence. A maintainer can follow an attempted operation through its initiating intent, Work and Home, exact Loopflow-authored and provider-submitted context, provider activity, usage, outcome, and observed consequence, with missing evidence stated honestly. This evidence supports execution diagnosis and justified prompting changes; it never selects priorities or grants execution authority. Intelligence owns evidence and context reliability, Infrastructure owns execution mechanics and repair, and Product owns human presentation and adoption. No remote telemetry service or opaque vendor-state reconstruction is required.

**T1.** Across fourteen consecutive days of real use, one supported local inspection path accounts for attempted core planning and execution operations, preserves their causal source identities, and distinguishes observed progress, intentional human waits, failures, and missing evidence. Twenty of twenty randomly selected settled agent Runs from the declared population reconstruct their required Loopflow-authored and provider-submitted context and connect initiating intent, Work where applicable, owning Home, provider activity, usage evidence, terminal result, and observed consequence without manual store joins. Required context includes exact submitted bytes and ordered component identities, sources, hashes, roles, and token accounting; absent required context fails reconstruction. Vendor-unavailable measurements remain explicitly unavailable. The readers remain usable against the long-lived migrated records, and the evidence neither substitutes successful process exit for delivered progress nor grants execution authority.

[Full proof contract and reconciliation](proposals/trace-context.md).

### Product / Company Dogfood

Reuse Project `d19956b2-9955-437d-aea6-d91766231c77`; rewrite `loopflow-api` without replacing identity.

Make Loopflow useful for advancing Cube, Etude, Kata, and Hootro: in every chapter week, at least three of these four have durable Work that accurately records the human's current desired next focus and observable material progress on at least one Task pursuing human-selected work. Humans select priorities; Company Dogfood judges the evidence and exposes missing intent, stalled work, and product friction. Chapter review and reset support that loop. Loopflow self-hosting, runtime settlement, and planning activity do not themselves establish external progress.

**G1.** In each of the four seven-day chapter windows, at least three of Cube, Etude, Kata, and Hootro have durable Work accurately stating the human's current desired next focus at the cutoff and dated, verifiable material progress during that window on at least one Task pursuing work selected by that human.

[Full proof contract and reconciliation](company-dogfood-reconciliation.md).

### Product / Desktop

Reuse Project `57fee17f-7231-46f3-afd8-031d866a1779`; rewrite `mac-surface-ux` without replacing identity.

Make the Mac desktop a dependable workspace for understanding human-selected Project/KR/Task intent and organizing the open Sessions that advance it. Build on provider-native continuation, retained panes, and explicit client transfer. Shared Loopflow APIs own planning, Work identity, legal actions, and human resolution; Desktop presents their evidence and preserves useful view state. Prove the promoted Ask handoff and truthful, responsive opening on the configured app, then join planning to Sessions for selected external work. Missing or stale evidence stays visible. Desktop owns user presentation, not a second transcript, evidence authority, runtime, or priority engine.

**D1.** Across ten recorded trials on human-selected work from Cube, Etude, Kata, or Hootro, one native workspace exposes current Project definitions, KR proof, and Task directives; round-trips at least one authorized Task-directive edit through the existing PM API; navigates between a Task and its exact open Sessions or an explicit no-Session state; and preserves useful panes and Work context. Shared Rust/Swift fixtures and twenty live Project, Task, and Session comparisons agree on identity, planning evidence, descriptive conditions, legal actions, and reasons. One real promoted Ask completes provider-native continuation, Ready, Complete, pane reconciliation, and blocked-caller release through the permissioned UI path. Desktop creates no second planning or Session authority.

**D2.** Across fourteen consecutive days of real use, at least twenty repository opens show the correctly scoped Sessions list and true count within a published p95 budget, without an empty-then-populate flash or manual repair, and at least twenty Session selections reach verified interactive embedded terminals within a published p95 budget, with serial-open position retained. Twenty cold-launch and planning-navigation trials on the long-lived registry meet published paint and interaction budgets. Missing or stale evidence never appears as healthy emptiness; delayed or failed selections stay explicit, and terminated clients never remain displayed as live.

[Full proof contract and reconciliation](desktop-reconciliation.md).

## Opening frontier and serial followers

These are allocation choices for Gate 2, not controller launches or runtime enforcement. Existing in-flight work survives; no worker is interrupted by this proposal.

| Project | Proposed opening Task | Subsequent selection in the same slot |
| --- | --- | --- |
| Reliability | LOO-279, finish existing OAuth recovery | One release-opportunity accounting and truthful-outcome Task |
| Architecture Minimalism | Existing `loopflow.wave-agents` change, under one tracked Task **after the execution-model/identity decision** | LOO-257; then one coherent repository-wide reduction/documentation pass. Placement can move first if it blocks the selected work. |
| Trace & Context | Capture required context and causal evidence at actual launch | Attempted-operation population accounting, then usable reconstruction and retained proof; do not wait for the window before completing readers. |
| Company Dogfood | Existing LOO-278, finish chapter slice and human demo/review | Weekly judgment through existing evidence. LOO-185 stays parked until a selected Discord workflow is blocked. |
| Desktop | LOO-280, supported-build Ghostty parity | LOO-251 → LOO-284 → one planning/Session integration Task. One serial slot. |

The Flow opening slot is conditional on the new architecture choice and exact existing Task association. Do not manufacture an ID or a second writer. Desktop’s packaging-first order is supported by source inspection, not a fresh build; a dated proof that a supported build already supplies the exact required path could justify LOO-251 first.

The proposed new planning Task is **Join current Project/KR/Task planning to native Sessions**. Its [exact directive](proposals/desktop.md) includes stable navigation, explicit absent/stale state, retained panes, one authorized Task-directive edit round trip, and scoped configured-path/budget proof. It excludes fleet restoration and a new lifecycle. No proposed new Task has been created.

## Every prior Project and open Task

| Prior Project | Stable Project ID | Proposed disposition |
| --- | --- | --- |
| Stability & Security | `03c59a52-dbd4-4e11-a6ac-f6d6359c8e08` | rewrite → Reliability (infrastructure) |
| Technical Architecture | `62b73cde-5057-4959-8ad8-fca96b9e80b5` | rewrite → Architecture Minimalism (infrastructure) |
| Performance & Efficiency | `3778bc54-6fcb-4f38-b262-8eb68bcaa602` | retire; preserve history and route retained obligations |
| Trace | `2f8390f7-6614-426c-84a8-fa5291358691` | rewrite → Trace & Context (intelligence) |
| Context | `0f37b71f-3c7c-43f4-b809-ca2c346ca5fa` | retire; preserve history and route retained obligations |
| Task-first control plane | `b211197b-4c71-4ea9-bef1-37f826ba5b5b` | retire; preserve history and route retained obligations |
| Loopflow API | `d19956b2-9955-437d-aea6-d91766231c77` | rewrite → Company Dogfood (product) |
| Mac Surface UX | `57fee17f-7231-46f3-afd8-031d866a1779` | rewrite → Desktop (product) |
| Auditability | `95159066-9098-4d0b-8903-01459dc7ec14` | retire; preserve history and route retained obligations |

Auditability obligations have explicit recipients: Desktop presents reasons and evidence; Infrastructure owns state semantics and repairs; Intelligence owns raw context/trace; Company Dogfood judges weekly external outcomes. List’s exact-authority, promotion-preservation and safe-signaling obligations remain Infrastructure acceptance conditions. Retirement never proves an expired KR held.

| Open Task | Disposition / owner | Allocation and trigger |
| --- | --- | --- |
| LOO-148 — Surface abandoned-Task recovery on the shared Now/Roadmap DTO | rewrite / Desktop | **parked**. Desktop owns presentation; Infrastructure Reliability owns shared recovery legality. Select only for reproduced blocked/abandoned recovery gap on selected external work; reconcile stale enum proposal first. |
| LOO-185 — Provision Loopflow's company Discord channels | rewrite / Company Dogfood | **parked**. Select only when the human chooses Discord for a named external workflow and provisioning friction blocks it. Preserve idempotence, identity and secret handling; no compulsory two-Home rollout. |
| LOO-251 — Finish the native Sessions multiplexer and promoted Ask handoff | carry / Desktop | **queued**. After LOO-280 in same slot: real Ask caller release, permissioned UI, split-paste confirmation, measured readiness and published budgets. Existing supported-build proof can justify earlier selection. |
| LOO-257 — Base new Task worktrees on current canonical main | rewrite / Architecture Minimalism | **queued**. Move to Architecture Minimalism; preserve technical contract, replace obsolete LOO-256 demo target. Select earlier if placement blocks selected work. |
| LOO-274 — Restore Wave fleet and roadmap loading in the Mac app | retire / retire | **retire**. Rejected general fleet restoration. Preserve code and history; do not reintroduce it under the scoped planning integration Task. |
| LOO-278 — Add chapter reviews and interactive plan resets | carry / Company Dogfood | **opening**. Finish existing chapter slice and working human demo/review. Gate 2 remains pending; no external-momentum credit. |
| LOO-279 — Make Linear OAuth recovery survive unattended Project runs | carry / Reliability | **opening**. Finish existing installed-path OAuth recovery; preserve Work, checkout, design and PR identity. |
| LOO-280 — Build one Ghostty-enabled Mac terminal product | carry / Desktop | **opening**. One Desktop slot: prove Ghostty parity in supported builds before required UI/Ask proof. Binary publication retains its normal authorization boundary. |
| LOO-281 — Prove shell blocks through the real shell, geometry, and long-output paths | carry / Desktop | **parked**. Select when reproduced shell-block behavior blocks human-selected work or the human prioritizes it. Preserve real-shell, geometry, copy, performance and Zig proof obligations. |
| LOO-282 — Show the recorded location of an active Session client | carry / Desktop | **parked**. Select when generic ELSEWHERE provenance obstructs a selected Session workflow; shared recorded provenance only, no inferred ownership. |
| LOO-283 — Evaluate shared Warp and Loopflow viewing without terminal regressions | carry / Desktop | **parked**. Explicit human reprioritization of simultaneous viewing activates this bounded investigation; retain disposable proof and allowed no-go, native default intact. |
| LOO-284 — Project Session actions and Work labels from the shared API | carry / Desktop | **queued**. After LOO-251, before planning integration: shared Session actions/Work labels and coherent Swift deletion. |

[Exact Task IDs and proposed directive changes](task-dispositions.json) preserve the LOO-185/148 rewrites and LOO-257’s replacement proof target. The unchanged technical acceptance criteria for carried work remain in the [frozen ledger](frozen-ledger.json).

Every one of the **42 exact prior KR claims** has one disposition in [the KR ledger](prior-kr-dispositions.md), including the three List claims without retrospective judgments. [Material review remainders](debt-dispositions.md) retain owners and concrete park triggers. No old claim inherits priority merely by existing.

The [historical Work ledger](historical-work-dispositions.md) gives one treatment to **174 unique Work records**, observed in 175 rows. LOO-57 appears twice under different Project views; both observations remain, but its disposition appears once. There are 119 PM-complete Tasks with residual nonterminal Work, 41 terminal Task records, two currently open Task Work records, nine current Project Work records, and three unavailable historical Projects. Propose retiring stale pursuit only after exact-state reconciliation and explicit Gate 2 acceptance; preserve every branch, PR, Run, artifact and historical ownership edge. Do not restore missing worktrees or infer shipment from PM completion.

## Challenges reconciled and choices returned to the parent

| Challenge | Root proposal / remaining decision |
| --- | --- |
| `loopflow.wave-agents` moved from generic Work advancement to Task-only execution with finite Wave/Project Runs | **Material human choice remains open.** Infrastructure ownership is settled; this implementation target is not. Reconcile the old review counterexamples, explicit continuation, lossless cutover, and existing Task association before selecting its exact directive. |
| Architecture child proposed five KRs | Combine into A1; preserve Flow safety and exact canonical-main placement in Task closure criteria, semantic reduction and actual weekly receipts in A1. [Rationale](architecture-reconciliation.md). |
| Company Dogfood child separated outcome and coverage | Combine into G1 with mandatory sixteen-row coverage and fixed weekly denominator. No credit for settlement, honest failure, planning churn, or Loopflow self-hosting. [Rationale](company-dogfood-reconciliation.md). |
| Desktop child proposed five KRs and challenged LOO-251-first | Combine into D1/D2 without dropping any of the five proof sets. Propose LOO-280 first, then one serial completion path. **Confirm packaging-first order and bounded planning-edit scope at Gate 2.** [Rationale](desktop-reconciliation.md). |
| Discord could expand scope into mandatory company setup | **Propose rewriting/parking LOO-185** until a human-selected external workflow needs it; terminal/native paths remain primary. Confirm this scope change at Gate 2. |
| A truthful failure could be called availability or momentum | Adopt stricter distinction: failures remain failures; ordinary usable-grant expiry and recoverable runtime failures must recover. Missing required context fails reconstruction; vendor-unavailable cost stays explicitly unavailable. |
| Release cadence has old weekly/nightly wording and current daily attempts | Propose retaining the installed configured schedule and all existing required verification. R1 uses actual due obligations and publication state, not process exits. Any schedule/surface change needs an explicit decision; none is proposed implicitly. |
| Execution/evidence ownership could split or duplicate | Reliability owns runtime repair; Intelligence owns attempted-operation evidence, including pre-Run failure and corrupt omissions. Product judges external results and presents shared state. No second monitoring Project or new authority. |
| New Tasks and proof windows exceed a small active frontier | Five proposed slots, serial within each Project; parked Tasks have triggers. Preserve unfinished KRs at review rather than silently increasing concurrency or reducing proof. |

**Parent Gate 2 decision:** accept or revise this exact five-Project/seven-KR portfolio, Task dispositions/parking, retirement ledger, dates, and opening frontier; explicitly resolve the Task-only execution target. Gate 1’s direction is not being reopened.

External focus remains an evidence gap: exact stable Work IDs and human-authored current priorities for Cube, Etude, Kata, and Hootro were not supplied or established by these scoped reads. Reuse existing unsuperseded human direction where available; return genuinely missing selections to the parent. Do not infer priorities from Session activity or invent external Work. Intelligence’s two-repository sample minimum does not select which projects move.

Numeric Desktop budgets remain implementation evidence work: publish defensible thresholds before scoring, never backdate them. The [proof calendar](proof-calendar.md) fixes four weekly windows; fourteen-day coverage must begin by 7 October 00:09:59 UTC to finish by review. Insufficient coverage remains unknown/unmet.

## Run receipts and preserved proposals

The root launched Infrastructure, then Intelligence, then Product only after the preceding Wave returned. Each Wave’s Project contributions were sequential. All eight child Runs completed, and none is unanswered. Intelligence’s provider reconnects recovered; raw partial emissions remain in its log, separate from the successful final report.

| Scope | Run ID | Complete archived report |
| --- | --- | --- |
| infrastructure | `run_fb04e36d5fe742c19463ba94385590cf` | [Original report](proposals/infrastructure.md) |
| reliability | `run_d9299390668b41d582405e990ee7e788` | [Original report](proposals/reliability.md) |
| architecture-minimalism | `run_84eafc507dd94ea9b382e293738adb58` | [Original report](proposals/architecture-minimalism.md) |
| intelligence | `run_5debbd828ff945eb86cc915cc626736e` | [Original report](proposals/intelligence.md) |
| trace-context | `run_3a453ab92c124ced8f5f6f380a2349fb` | [Original report](proposals/trace-context.md) |
| product | `run_ea98cc7ad3ac40908b01ca0e8455f724` | [Original report](proposals/product.md) |
| company-dogfood | `run_dfe0cc8351914375b3769a953cad3cfe` | [Original report](proposals/company-dogfood.md) |
| desktop | `run_787dbbe691c44d5d91ea7cf94575cde2` | [Original report](proposals/desktop.md) |

[Terminal receipts and exact parent lineage](launches/run-index.json), launch briefs, captured output and per-Wave command receipts are retained. File paths in the original reports refer to their observation surfaces; this archive preserves their complete content and does not silently rewrite their judgments.

## Application boundary and verification

No PM writes, Work lifecycle changes, Wave charter edits, pushes, PR operations, or chapter sealing were performed. All application statuses remain `not_attempted`; `proposal.json` has an empty applied-operation ledger. Local archive/report files are the only deliverable changes.

After explicit Gate 2 acceptance, application must preserve stable Project IDs, reconcile name-derived references, move/rewrite selected Tasks before retiring their former containers, and treat stale Work by exact ID after checking current active artifacts. Engbot’s provider state must be confirmed before its proposed retirement. Stop on a mismatch rather than overwrite the accepted record or disrupt active work. Charter/metric edits follow the Task PR path; live planning uses its existing owner. This packet does not authorize those operations.

The packet audit checks exact starting-object coverage, unique dispositions, all child completions and ordering, unchanged source hashes, and no sealed `start.md`. No production code changed, so no build or behavior-test pass is claimed. [Verification record](validation.json).

## Human Gate 2 acceptance and correction — 2026-09-23

The human explicitly accepted the complete Gate 2 packet in the live parent conversation on 2026-09-23, as relayed in this application Run's instruction. No separate Ask receipt or more precise acceptance time is claimed. Apply the five-Project/seven-KR portfolio, all prior-object dispositions, the fixed chapter/review dates, and the serial frontier through existing owners. This dated decision supersedes pending language and packaging-first recommendations above; original child reports and frozen observations remain unchanged.

Desktop opens with LOO-251. LOO-280 occupies the same serial slot only if real configured LOO-251 proof is blocked by the Xcode/Ghostty build gap. Then LOO-284 precedes the accepted planning/Session integration Task. No source-only prerequisite claim activates LOO-280.

Wave, Project, and Task all remain durable Work. Long-lived implementation pursuit and PR ownership belong to Tasks; Wave and Project operations are bounded Runs against their durable records. Preserve the Flow change's recorded preservation and exact-authority counterexamples and reconcile its existing Task association before any launch.

LOO-185 remains rewritten and parked as proposed. External human focus/Work IDs and numeric Desktop budgets remain evidence work, not inferred selections or invented thresholds. Preserve stable IDs, active artifacts, and history. Seal only after every accepted operation succeeds and live state verifies. Local `lf commit` is authorized; push, publish, submit, arm, and land are not.

## Application complete — 2026-09-23T16:13:45.110901+00:00

Human Gate 2 acceptance: 2026-09-23, explicitly relayed by the live parent instruction. All accepted operations succeeded and live sources were verified before sealing. The fixed review remains 2026-10-21T00:09:59.226362+00:00.

- Five existing Projects retain their UUIDs with accepted definitions and seven unproven KRs; four other Projects are archived.
- All twelve prior open Tasks have applied dispositions; LOO-257 moved to Architecture Minimalism, LOO-274 closed explicitly as retired, and LOO-185 remains parked. Eighteen Tasks are now open, including seven newly created accepted Tasks.
- All 42 prior KR claims retain their exact text, historical verdicts, and explicit dispositions.
- All 174 unique historical Work dispositions verified: 119 stale Task pursuits and seven Project pursuits abandoned; 41 terminal Task histories, two active Tasks, and five retained Project Work records carried.
- List and Engbot Wave Work are Abandoned and disabled; their registrations/history remain. The current Task checkout retains the three accepted charters and removes List from the authored roster.
- Obsolete task-loop-trust contract bytes are archived; historical observations are untouched.
- No Task or Project controller launched, no active work restarted, no branch/PR/Run history deleted, and no push or PR publication performed.

Desktop opens with LOO-251. LOO-280 is conditional on a demonstrated Xcode/Ghostty blocker in that configured proof and uses the same serial slot. Then LOO-284 precedes LOO-291. Wave, Project, and Task all remain durable Work; long-lived implementation pursuit and PR ownership belong to Tasks, while Wave/Project operations are bounded Runs against durable records.

New planning identities:

- LOO-285 / `049348f6-de52-4ac9-a94c-5c0804435668` — Account for scheduled release opportunities and settle actual release outcomes (Queued behind LOO-279 in the same serial Reliability slot.)
- LOO-286 / `a30ea5c3-028c-41a6-93eb-5a8a285f4c01` — Finish Task-owned Flow execution with one authority and lossless cutover (Opening Architecture allocation for the existing implementation; no controller launch authorized by this planning operation.)
- LOO-287 / `e3f42e6a-885e-4d15-b7b5-bdccf68e00bd` — Reduce competing concepts and align the reviewed architecture (Queued behind the opening authority change and LOO-257; select earlier only for a demonstrated blocker or competing authority in those proofs.)
- LOO-288 / `fb2f2d99-6019-4b09-a738-7c95a655a048` — Capture required context and causal evidence at the actual launch boundary (Opening; one serial Intelligence slot.)
- LOO-289 / `5cb72e9c-a920-4441-900c-51d227b51669` — Account for attempted operations and their observed outcomes (Queued in the same serial Intelligence slot after the preceding capture/population Task. Retaining evidence does not prevent selecting the next Task.)
- LOO-290 / `6e89912a-36e5-46fa-960a-fa48a7be5142` — Make reconstruction usable through the supported inspection path (Queued in the same serial Intelligence slot after the preceding capture/population Task. Retaining evidence does not prevent selecting the next Task.)
- LOO-291 / `ee671927-255f-41e6-8429-b830d59cc1de` — Join current Project/KR/Task planning to native Sessions (Queued after LOO-251 and LOO-284 in the same serial Desktop slot. Activation also requires a named human-selected external proving workflow; do not infer one. LOO-280 is conditional only on the real configured LOO-251 proof being blocked by Xcode/Ghostty.)

Proof: [application validation](application/validation.json), [all owner receipts](application/operations.jsonl), [exact Task directives](application/expected-tasks.json), [new Tasks](application/new-task-plan.json), [historical reconciliation](application/historical-reconciliation.json), and [stable Project references](application/project-references.json).

Observed limitation: the legacy Project lifecycle projection retains last-execution plan labels and does not resolve new slugs yet. Use the recorded stable UUIDs; `lf pm show` and shared Wave status hold the accepted current planning. The failed name probe is preserved separately from successful application receipts. No controller restart or direct store rewrite repaints that history.

Repository charter/roster/metric changes are prepared and verified in this Task checkout. Canonical deployment waits for the human's publication decision. External focus/Work IDs and Desktop numeric budgets remain unproven evidence work; the chapter does not invent them. No KR achievement is claimed by applying a plan.
