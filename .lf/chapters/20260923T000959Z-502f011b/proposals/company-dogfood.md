# Company Dogfood — proposed chapter

Chapter: `20260923T000959Z-502f011b`  
Proposal Run: `run_dfe0cc8351914375b3769a953cad3cfe` (read from `LF_RUN_ID`)  
Root Run: `run_502f011b9d1f4596ba62eff0e54a8b0e`; parent Task: LOO-278  
Next review: **2026-10-21T00:09:59.226362+00:00**  
Status: **Proposal only. Gate 2 is not accepted.** Gate 1 direction is supplied by the parent; this report claims no further human acceptance.

**Verdict: agree** with reusing Loopflow API as Company Dogfood under Product. Its proof must concern Cube, Etude, Kata, and Hootro. **Challenge** any interpretation that credits Loopflow planning, a successful Run, or an honest failure as external momentum. The current metric would permit precisely that substitution.

## Proposed identity and exact definition

Retain Project UUID `d19956b2-9955-437d-aea6-d91766231c77` and Project Work `proj_9f1fc19a84211aed8b8136e55c8ab1f3`. Proposed name: **Company Dogfood**; proposed slug: `company-dogfood`; sole parent: Product (`5081a624-c095-4218-9a54-6d1a6711f282`). Preserve existing Task, PR, and Run history. Rename through the existing PM owner only after Gate 2; reconcile slug references without creating a replacement Project.

Exact proposed definition:

> Make Loopflow useful for advancing Cube, Etude, Kata, and Hootro: in every chapter week, at least three of these four have durable Work that accurately records the human's current desired next focus and observable material progress on at least one Task pursuing human-selected work. Humans select priorities; Company Dogfood judges the evidence and exposes missing intent, stalled work, and product friction. Chapter review and reset support that loop. Loopflow self-hosting, runtime settlement, and planning activity do not themselves establish external progress.

## Exact proposed numbered KRs

1. **In each of the four seven-day chapter windows ending 2026-09-30, 2026-10-07, 2026-10-14, and 2026-10-21 at 00:09:59.226362 UTC, at least three of Cube, Etude, Kata, and Hootro have both durable Work accurately stating the human's current desired next focus at the cutoff and dated, verifiable material progress during that window on at least one Task pursuing work selected by that human.**
2. **At every weekly cutoff in this chapter, the evidence record accounts for all four external projects, identifies the human-direction source and relevant Work/Task evidence or their explicit absence, and distinguishes demonstrated progress, observed non-progress, and unknown coverage. Every credited project-week has resolvable proof of both current focus and material Task progress; missing intent, failed attempts, and inaccessible or contradictory evidence are never omitted or credited as progress.**

Both proposed KRs begin **unknown**. Neither is proven by this proposal or the completed baseline review.

## Measurement contract

**Population and interval.** Each week has exactly four members: Cube, Etude, Kata, Hootro. The denominator never shrinks for unavailable Homes, missing Work, inactivity, missing human direction, or failures. Loopflow and Cadenza never enter it. A project contributes at most one success per week, regardless of Task or Run count. The chapter has sixteen project-week rows; passing twelve overall is insufficient if any individual week has fewer than three.

Use half-open UTC windows anchored at `2026-09-23T00:09:59.226362Z`, with the four cutoffs above. Attribute a progress event exactly at a cutoff to the next window. Do not move the anchor to hide a bad week. These are proposed measurement dates, not a claim that Gate 2 has occurred. If final acceptance happens after the opening boundary, earlier coverage must be supported by contemporaneous records; otherwise disclose unknown coverage and return any date change to the parent for acceptance.

**Current human focus.** Cite an explicit human-authored or human-accepted direction, its timestamp, the external repository, and stable Wave/Project/Task identities. Durable Work at the cutoff must accurately reflect the latest applicable direction, including any pause or redirection. Existing direction may remain current without weekly reapproval when it has not been superseded; task rank, agent inference, and absence of a reply are not new priority decisions. Preserve changes of focus with their effective times. Progress counts only when it pursued the human-selected focus then in force; a later focus change cannot retroactively authorize unrelated work.

**Material progress.** Require a dated change in the external Task's substantive outcome, with evidence a maintainer can inspect: a shipped or demonstrably working behavior; a reviewable implementation reaching an explicit Task milestone with behavioral proof; or an investigation/document Task's verified answer or usable artifact satisfying its stated acceptance criterion. A merged PR alone is insufficient without its connection to the intended outcome. Human acceptance of an artifact can provide proof where judgment is inherently subjective.

Task creation, status movement, scheduling, tokens, activity logs, branch existence, chapter reports, focus resets, and successful Run exit are not material progress by themselves. A failure receipt is also insufficient. A failed Run may have produced independently verified material progress; credit that specific result, retain the failure, and never credit merely having diagnosed a failed attempt. An investigation's negative result can count only when it answers the external Task's actual question with evidence, not when routine execution fails.

**Missing focus and failures.** Known absence of human direction or durable Work is observed non-qualification, not an invitation to invent priorities. Unreachable or stale sources that prevent judging focus remain unknown. A human pause remains visible in the denominator; a paused project without progress does not qualify. Auth errors, pre-Run failures, stalls, manual rescues, and missing traces remain in the evidence record. Repairs to Loopflow belong to their supporting owners and do not substitute for external Task outcomes.

**Verdict arithmetic.** For each project-week report focus and progress separately as proven, observed unmet, or unknown, with evidence and gaps. It qualifies only when both are proven. A week holds with at least three proven qualifiers; it does not hold when proven qualifiers plus unknown candidates cannot reach three; otherwise it is unknown. KR 1 holds only after all four windows hold. A known failed week establishes that KR 1 does not hold; without one, any unresolved window keeps it unknown. Report unresolved fourth-project evidence even when three proven qualifiers establish the threshold. KR 2 requires all sixteen rows and honest coverage, not sixteen successful projects.

This is a review contract using existing Work and evidence, not a new runtime metric authority, controller, or priority engine. Small/Medium/Big remain review heuristics only.

## Prior numbered KR dispositions

Baseline verdicts below are carried forward verbatim; the manual review was not rerun. Retirement never changes an unknown or failed result into success.

| Prior KR | Exact prior text | Baseline | Proposed disposition |
|---|---|---|---|
| 1 | The thesis holds under load: >= 5 waves run consistently for 1 week straight across loopflow and Cadenza, at least 2 per codebase driven from GOAL.md with zero repo-authored skills added for the work. | unknown | **Retire.** Replace its portfolio premise with proposed KR 1; remove the five-wave, Cadenza, and zero-authored-skills commitments. |
| 2 | Task loops earn trust by streak: over one week of real work, every dispatched task loop either lands its PR unattended or stops with an actionable non-convergence record — zero silent stalls, zero human rescues inside the window. | unknown | **Retire from this Project.** Infrastructure / Reliability owns execution settlement and recovery; it must shape its own accepted proof. Do not copy this KR as a Company Dogfood success measure. |
| 3 | The spawn chain survives a week: wave loops spawn project loops spawn task loops with caps enforced throughout, and no run escapes its budget. | unknown | **Retire.** Do not preserve a topology promise or translate review size heuristics into runtime caps. Task-only execution with finite Wave/Project Runs remains a material Gate 2 decision, not an accepted consequence of this retirement. |
| 4 | One model everywhere, continuously: for a month of landings, CLI, Mac, iOS, and agent prompts expose the same wave/project/task model — a parallel concept appearing anywhere is a failure event. | fail | **Retire as written.** Drop the iOS and month-long universal promise. Desktop owns relevant user presentation; Infrastructure owns shared semantics and repair. Consistency remains a design obligation, not evidence of external momentum. |
| 5 | A new wave can be created, steered, delegated, inspected, and closed through the shared API contract, repeated cold 3/3 times. | unknown | **Retire.** Actual external use supplies the relevant evidence. Do not impose ceremonial creation/closure or a new three-run lifecycle gate. |

The existing `product/task-loop-trust` contract still binds this stable Project UUID. **Propose retiring that contract from Company Dogfood during accepted application**, preserving its observations and unknown baseline. Reliability may adopt an explicitly reconciled successor; Intelligence may supply evidence. Renaming the Project must not silently relabel this installed, uninstrumented, never-observed settlement metric as external momentum.

## Every starting open Task

| Task and stable ID | Disposition | Proposed owner, opening status, and trigger |
|---|---|---|
| LOO-278 — Add chapter reviews and interactive plan resets (`eef88fa6-ddcf-4570-a779-8110c7a161c6`) | **Carry** | Company Dogfood; sole opening Task, already in flight. Finish the existing coherent review/reset slice and human demo/review boundary. Preserve the two acceptance gates and existing work. Its output supports capture of human-selected external focus but earns no external momentum credit itself. Do not append a new portfolio engine or unrelated reporting implementation. |
| LOO-257 — Base new Task worktrees on current canonical main (`2ada2b81-ffe1-40cb-9544-8c7bcb613c66`) | **Rewrite ownership / transfer; preserve technical directive** | Infrastructure / Architecture Minimalism. No Company Dogfood execution slot. Proposed parked behind the existing `loopflow.wave-agents` commitment; receiving owner may select it when a reproduced placement refusal blocks human-selected work, or at its next frontier review. No completion is claimed. |
| LOO-185 — Provision Loopflow's company Discord channels (`2c3d04d1-91b9-45ab-9ee5-2488f97d8aad`) | **Rewrite; park** | Company Dogfood. Activate only when a human chooses Discord for a named external workflow and observed provisioning friction blocks that workflow. No mandatory onboarding, no speculative two-Home rollout. Preserve channel identity, idempotence, secret handling, and permission requirements for the selected slice. Revisit at the next review if no trigger occurs. |

Exact proposed LOO-185 title: **Provision Discord channels for a human-selected external workflow.** Exact proposed outcome: **When the human selects Discord for Cube, Etude, Kata, or Hootro, idempotently provision or adopt the channels needed for that named workflow, preserve stable bindings through rename and retry, and demonstrate the workflow on its actual repository/Home configuration. Remain parked until that need is evidenced; CLI and native Sessions remain valid operating paths.**

## Opening frontier and adjacent ownership

Keep Company Dogfood at **one active Task: LOO-278**. After its review boundary, do not automatically activate LOO-185. Weekly judgment can use existing records without creating a new implementation Task. If those records cannot establish the KR, preserve the gap and propose only the smallest concrete product repair against an already human-selected external Task. Do not select which external projects move next.

The following are boundary recommendations for parent reconciliation, not changes to another Project's plan:

- **LOO-251: carry to Desktop**, preserving validated native Sessions and its remaining configured-path proofs; no Company Dogfood slot.
- **LOO-280: carry to Desktop**, activation only if build parity blocks its selected configured proof; otherwise parked.
- **LOO-281: carry, parked in Desktop** until shell blocks are selected as an observed external workflow impediment or the human explicitly prioritizes them.
- **LOO-282: carry, parked in Desktop** until client-location ambiguity obstructs a selected Session workflow.
- **LOO-283: carry as a parked Desktop investigation**, activated only by explicit selection of the shared-view experiment; no implied runtime or terminal redesign.
- **LOO-284: carry, parked in Desktop** until its selected planning/Session work needs shared action and Work-label projection; do not add a separate evidence/control authority.
- **LOO-274: retire**, following the supplied rejection; do not revive broad fleet loading through this bet.
- **LOO-148: rewrite under Desktop as the single accountable Task owner; park.** Infrastructure owns the shared recovery legality it consumes. Trigger: a reproduced abandoned-Task recovery gap blocks human-selected external work or Desktop's selected planning path. Reconcile the old actionability proposal with the current shared action model before implementation; no iOS commitment or independent lifecycle model. Parent must resolve any competing owner proposal explicitly.
- **`loopflow.wave-agents`: route to Infrastructure / Architecture Minimalism**, outside this frontier. Its Task-only execution proposal still requires Gate 2.
- **LOO-279: Infrastructure / Reliability**, outside this frontier; do not count its self-hosting repair as external progress.

## Dependencies and material questions returned to the parent

Desktop presents Project/KR/Task intent and organizes open Sessions without becoming an evidence or execution authority. Infrastructure owns release/auth/continuity, flow-driven advancement, shared semantics, and architecture reduction. Intelligence owns raw trace/context/provenance and preserves pre-Run failures, missing context, and corrupt omissions. Its proposed twenty-Run and fourteen-day populations support diagnosis; they do not supply Company Dogfood's numerator. Account juggling and prompting work need an observed connection to the selected external workflow.

Retiring Auditability must preserve these explicit recipients: Intelligence for raw evidence; Infrastructure for semantics/repair; Desktop for presentation; Company Dogfood for weekly external momentum judgment. This proposal assumes no new active Task for that transfer.

Return these material choices for Gate 2: accept the exact fixed weekly measurement contract and proposed dates; reconcile adjacent Task parking with Desktop's chosen frontier; explicitly settle the Task-only flow change separately; and obtain human-selected external focus where it is missing. This Run chooses no external priorities and opens no human Session.

## Evidence and limits

Read-only inspection on 2026-09-23 UTC found:

- Cache-only `lf pm show --wave product --project loopflow-api --no-sync --json`, synced **2026-09-22T22:38:59Z**, retains the supplied definition, five KRs, and exactly three open Tasks. It is newer than the prompt snapshot; no scoped ledger divergence was found.
- Frozen ledger captured **2026-09-23T00:09:59.229630+00:00** and supplied Product status preserve LOO-278's active PR plus `ready` runtime and `blocked: Task has uncommitted work` condition. Current status at approximately **01:35:57 UTC** still reports these. “Already in flight” therefore does not mean clean, done, or unattended success. LOO-257 and LOO-185 have no runtime and are ready to start; that is not a priority decision.
- `lf roadmap --json` from this repository exposed Product, not an external four-project evidence population. No conclusion about external absence or progress follows from that scoped read. External human-focus and Task-outcome evidence remains **unknown** here.
- `rust/loopflow/src/lf/commands/waves.rs` exposes roadmap Work, runtime, Task conditions, PR references, and unavailable evidence. These support inspection; none establishes material external progress without outcome evidence.
- `wave/product/metrics/task-loop-trust.md` excludes open Tasks and permits non-resumable failures in its successful-settlement numerator. It is unsuitable for the proposed outcome. Baseline uninstrumented/never evidence stays unknown.
- Builtin `start-chapter.md` and `review-chapter.md` preserve two human gates, proposal-only child boundaries, historical evidence, and explicit unknowns. Code presence does not prove the full LOO-278 demo.
- The supplied manual baseline HTML and ledger were used as completed evidence, not rerun. Historical verdicts remain four unknown and one fail. Product memory was read; its earlier broad surface and runtime commitments do not override supplied Gate 1 direction.

Source archive: `/Users/jack/src/loopflow.chapter-planning/.lf/chapters/20260922-manual-baseline/`; frozen ledger and Product status: `/Users/jack/src/loopflow.chapter-planning/.lf/chapters/20260923T000959Z-502f011b/`.

Coverage: complete for this Project's five prior KRs and three starting open Tasks; adjacent named Tasks have explicit boundary recommendations for the parent. External outcome proof is incomplete and remains unknown. No PM, Work, charter, implementation, checkout-file, commit, push, controller, or auth changes were made. The complete original report is saved at `/tmp/product-company-dogfood-original.md`; the parent alone owns its tracked archive and any accepted application.
