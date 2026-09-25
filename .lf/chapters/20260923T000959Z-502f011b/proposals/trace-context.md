**Agree: consolidate into one Trace & Context Project, retaining Trace’s identity.** Exact context, Run explanation, and Reliability measurement depend on the same evidence chain. A separate monitoring Project would duplicate ownership before that chain is dependable.

This is a **Gate 2 proposal**, not an accepted or applied plan. No files, PM records, Work state, or charters were changed; no child Runs, human sessions, implementation, or population audits were started.

Chapter: `20260923T000959Z-502f011b`  
Next review: **2026-10-21T00:09:59.226362+00:00**

**Project dispositions**

| Existing Project | Proposed disposition |
|---|---|
| Trace — `2f8390f7-6614-426c-84a8-fa5291358691`; Work `proj_9fffbe63b7594bdc8f5719c61b25876d` | **Carry and rewrite.** Rename to **Trace & Context**, retaining Project and Work identity, under Intelligence. Reconcile name-derived references during application. |
| Context — `0f37b71f-3c7c-43f4-b809-ca2c346ca5fa`; Work `proj_c8a7e002f6cc4830893d540a93a1db09` | **Retire as a separate Project and retire its residual execution intent.** Map its obligations below to Trace & Context, Product, Infrastructure, or explicit parked triggers. Preserve historical attribution and completed Tasks. |
| Intelligence Wave | **Retain.** This proposal changes its Project portfolio, not its accepted boundary. |

Do not rewrite historical Context Runs as though Trace originally caused them. Consolidated queries can follow the explicit successor relationship while preserving original Project, Work, Run, and Home identities.

Proposed exact definition:

> Human-selected portfolio work is explainable from durable local evidence. A maintainer can follow an attempted operation through its initiating intent, Work and Home, exact Loopflow-authored and provider-submitted context, provider activity, usage, outcome, and observed consequence, with missing evidence stated honestly. This evidence supports execution diagnosis and justified prompting changes; it never selects priorities or grants execution authority. Intelligence owns evidence and context reliability, Infrastructure owns execution mechanics and repair, and Product owns human presentation and adoption. No remote telemetry service or opaque vendor-state reconstruction is required.

**Exact proposed KR ledger**

One KR, initially **unchecked**:

> Across fourteen consecutive days of real use, one supported local inspection path accounts for attempted core planning and execution operations, preserves their causal source identities, and distinguishes observed progress, intentional human waits, failures, and missing evidence. Twenty of twenty randomly selected settled agent Runs from the declared population reconstruct their required Loopflow-authored and provider-submitted context and connect initiating intent, Work where applicable, owning Home, provider activity, usage evidence, terminal result, and observed consequence without manual store joins. Required context includes exact submitted bytes and ordered component identities, sources, hashes, roles, and token accounting; absent required context fails reconstruction. Vendor-unavailable measurements remain explicitly unavailable. The readers remain usable against the long-lived migrated records, and the evidence neither substitutes successful process exit for delivered progress nor grants execution authority.

The following are acceptance rules for that **one KR**, not additional monitoring or optimization KRs:

- **Declare the population before sampling.** Name the Homes, repositories, operation types, launch surfaces, interval, and sampling method. Include real use from at least two of Cube, Etude, Kata, and Hootro. An insufficient population means “not yet demonstrated.”
- **Account for attempts before successful capture.** Reconcile operation-entry evidence with Runs and consequences, including attempts that fail before a provider launches. A denominator built only from existing Run bundles cannot establish coverage.
- **Do not select around missingness.** Known missing required context in newly captured Runs within the declared window prevents acceptance, even if twenty other Runs pass. Preserve older incomplete records as historical failures; do not backfill them from current ambient context.
- **Scope context honestly.** Reconstruct what Loopflow authored or submitted, including applicable durable direction, design/scratch, observations, and recorded dynamic input. Do not claim access to provider-internal instructions or opaque working state. Missing a replay-specific launch contract is not automatically missing launch context.
- **Exercise handoffs and diagnosis.** Include available restarted or multi-pass Tasks and failed or heavily steered Runs in the declared sampling strata. Check that current durable direction reached the applicable launch and that a maintainer can identify the evidence behind a diagnosis. Retain receipts for any resulting prompting repair and its follow-up Run.
- **Separate measurement from availability.** Report progress, waits, failures, unknowns, coverage, and their denominators separately. An actionable failure remains a failure. Explicit missingness can make the measurement honest while leaving Reliability’s availability KR unproven.
- **Keep reader failure visible.** Historical schema incompatibility, malformed records, inaccessible directories, and truncated listings must not disappear from completeness claims. Preserve affected record identities and actionable errors.

The fourteen-day view is **acceptance evidence under Trace & Context**, consumed by Infrastructure Reliability. It does not warrant a second Project, dashboard program, success score, or automated recovery authority.

**Evidence supporting the proposal**

The [completed baseline](/Users/jack/src/loopflow.chapter-planning/.lf/chapters/20260922-manual-baseline/chapter-review.html) supports both consolidation and unfinished work:

- The twenty-launch context audit passed, demonstrating a useful existing capability.
- Among 82 settled records, six lacked context and 35 lacked launch contracts.
- Among 80 settled usage rows, 21 lacked tokens and 76 lacked cost.
- No recorded replay-operation cohort supported the broad replay promise.
- Three qualifying prompting/context landings lacked motivating and follow-up Run identifiers.

These are different populations and should remain separate.

Current code establishes the implementation boundary:

- [runs.rs](/Users/jack/src/loopflow/rust/loopflow/src/lf/commands/runs.rs) and [usage.rs](/Users/jack/src/loopflow/rust/loopflow/src/lf/commands/usage.rs) read Home-local Run bundles. Usage explicitly does not claim interval completeness or provider finality. The Wave memory’s `run_events`/`own_spend` descriptions are stale.
- [run_record.rs](/Users/jack/src/loopflow/rust/loopflow/src/run_record.rs:802) validates a supplied context reference but accepts an absent reference in that integrity helper. Consequently, a zero-gap summary alone does not prove required context exists.
- [trace.rs](/Users/jack/src/loopflow/rust/loopflow/src/trace.rs:249) already represents exact prepared context and component accounting. Extend the real capture boundary rather than inventing another context store.
- [replay.rs](/Users/jack/src/loopflow/rust/loopflow/src/lf/commands/replay.rs) launches a recorded request through the ordinary harness using the recorded working-directory path. Its existence does not demonstrate historical environment reproduction or the old 10/10 unattended promise.

Current `lf status intelligence --json` confirms no open PM Tasks, no listed Wave Runs, and a stopped resident Home runtime. Trace and Context retain Ready Project projections. That is current operational evidence, not proof that their KRs hold or that all historical activity is absent.

**All eleven old KR dispositions**

Numbering follows the supplied ledger: Trace 1–5, Context 6–11. Historical verdicts remain unchanged.

| # | Old obligation and verdict | Proposed disposition |
|---|---|---|
| **1** | Month-long complete Run reconstruction; **fail** | **Rewrite into the new KR.** Preserve context, intent, causal shape, time, result, and available usage. Replace the unsupported universal month claim with a declared fourteen-day population and twenty-Run proof. Missing required context fails; unavailable vendor cost is explicit. |
| **2** | Month of successful `runs`/`trace`/`usage` readers; **fail** | **Rewrite into reader acceptance.** Use current supported surfaces, including `lf runs <run>`. Exercise long-lived migrated records and retain failures. Retire the obsolete command contract and unobserved historical month claim. |
| **3** | Universal provider/token/cost coverage and no unexplained silence; **fail** | **Rewrite into population and attribution evidence.** Preserve direct usage ownership, finality, and causal identities. Reconcile attempted operations with records; do not infer outages solely from quiet days. Unavailable cost is not zero, and Run settlement does not make provider usage final. |
| **4** | Any recent Run replays unattended, 10/10; **fail** | **Retire from the active chapter; park.** Preserve shipped replay capability and historical evidence. Reopen only for a named debugging need requiring replay, with a bounded provider/environment contract and authorized execution. |
| **5** | One-query hotness, cost, and trends; **fail** | **Narrow into supported inspection and the Reliability population view.** Keep direct usage and coverage visible. Park broad trends, cost-per-delivery, dashboards, and optimization claims until a concrete decision needs them and comparable evidence exists. |
| **6** | Every qualifying landing has motivating/follow-up Runs and no gate regression for thirty days; **fail** | **Rewrite as the evidence obligation for prompting repairs undertaken in this bet.** Retain motivating evidence, the intended change, follow-up behavior, and relevant gate result. Park repository-wide universal enforcement until qualifying landings and gate outcomes can be joined completely. |
| **7** | Exact context plus thirty-day total/per-layer budget compliance; **fail**, with reconstruction subproof passed | **Carry exact context into the new KR; retire the unsupported budget conjunct.** Preserve sources, identities, hashes, ordering, roles, and token accounting. Make actual omission/truncation decisions inspectable. Reopen numeric budget tuning only after measured pressure identifies a specific problem; never silently drop required context to meet a budget. |
| **8** | Ten-Task intent-preserving handoffs; **unknown** | **Rewrite into sampled handoff acceptance.** Verify current direction, design, observations, and slice in applicable launch evidence. Intelligence owns proving what arrived; Infrastructure owns restart/advancement mechanics. Do not claim that supplied context guarantees agent compliance with every invariant. |
| **9** | Lower input tokens without gate regression across adjacent-window cohorts; **unknown** | **Retire from the active chapter; park.** Trigger: stable comparable cohorts with delivery/gate joins and a specific measured context-cost problem. No token-reduction target now. |
| **10** | Three zero-config repositories land PRs; **unknown** | **Route adoption proof to Product; retain context defects in Intelligence.** Product chooses external journeys and judges usability. Trace & Context supplies launch/configuration evidence and repairs demonstrated context failures. Do not carry the unobserved three-repository or thirty-day no-knob claim. |
| **11** | Seven-day Discord disposition/relevance guarantees; **unknown** | **Retire the Discord/cadence-specific KR.** Preserve provenance and relevance for messages actually selected into context. Product owns the human communication contract; Infrastructure owns delivery mechanics. Reopen a scoped routing audit when an accepted input path or observed lost/irrelevant input requires it. No raw cross-Wave transcript becomes ambient context. |

This mapping preserves Context’s required-context, provenance, handoff, and prompting-diagnosis obligations. Retiring its Project does not declare those obligations complete.

**Historical Tasks and Work reconciliation**

The frozen ledger contains **25 PM-complete Tasks and zero open PM Tasks** across the two Projects. No existing open Task needs carry or rewrite. New work should address present evidence gaps without reopening completed umbrella Tasks.

| Historical Tasks | Proposed disposition |
|---|---|
| LOO-272, LOO-271, LOO-245, LOO-129, LOO-132, LOO-135, LOO-137, LOO-138, LOO-139, LOO-140, LOO-143 | **Keep complete.** Preserve historical ownership and evidence. Completion does not certify current behavior or endurance KRs. |
| LOO-130, LOO-131, LOO-133, LOO-134, LOO-136, LOO-142, LOO-144, LOO-145, LOO-146, LOO-147 | **Keep complete and absorbed.** Preserve successor relationships; do not recreate their former scopes as parallel Tasks. |
| LOO-141 | **Keep complete and retired.** |
| LOO-267 | **Keep PM-complete and Work Abandoned.** Preserve Work `task_c89277ebd41a41409fbf943524fa99de` and PR record `pr_f4aad1af594045d6a2ac2a71a53bc730`. Its missing checkout does not justify restoration or restart. |
| LOO-246 | **Keep PM-complete; retire residual Ready Work** `task_630c7f1c1b6347ea8c31d99cf567cfb2`. Preserve unpublished working PR record `pr_711f255657d4433da2eff3691a5d72b7` and missing-checkout evidence. Do not infer a merge. |
| LOO-128 | **Keep PM-complete; retire residual Ready Work** `task_1b4e3f4279994c6ea611aebf64a7baa8`. Preserve unpublished working PR record `pr_15e37f8e911d467b859d6f4fa4efa037` and missing-checkout evidence. Do not restart the broad old implementation mandate. |

These are proposed dispositions for parent application, not deletions of branches, artifacts, PR history, or evidence. The Wave’s `active_tasks: 2` must not be interpreted as two current PM commitments.

**Small sequential opening Task frontier**

Propose three Tasks by title only; no IDs are created or requested here. Initially activate **one**, with the others queued.

1. **Capture required context and causal evidence at the actual launch boundary.**  
   Use the baseline’s known gaps to identify affected real launch paths. Preserve exact prepared context, component provenance, initiating source, Home, applicable Work, parent Run, and provider-attempt identity. Distinguish missing required context from optional replay material and unavailable vendor measurements.  
   **Done when:** changed paths produce inspectable local records through the configured harness; a fresh/restarted launch demonstrates current durable direction; absence or corruption of required context cannot receive a reconstructed verdict. Include a pre-provider failure case. Correct stale evidence-contract guidance with the implementation.

2. **Account for attempted operations and their observed outcomes.**  
   Establish the bounded population with Reliability using actual operation-entry and consequence evidence. Include attempts that never create Runs, explicit human waits, failures, retries, and missing outcomes without double-counting them. Start retention as soon as this boundary exists.  
   **Done when:** every included attempt is represented with causal source identity and a defensible classification; counts and denominators reconcile; missingness remains visible; failures link to evidence and an owning repair disposition where one exists. Do not wait fourteen days before starting the next Task.

3. **Make reconstruction usable through the supported inspection path.**  
   Connect the captured evidence through current readers, with Product reviewing the human-facing contract. Avoid requiring manual filesystem/SQLite joins.  
   **Done when:** retain the declared twenty-Run reconstruction sample and fourteen-day population, demonstrate diagnosis and applicable handoffs, and report every unmet acceptance condition explicitly. Repair reader/capture defects within Intelligence; route runtime failures to Infrastructure.

This sequence narrows the baseline’s three overlapping seed Tasks into capture, population accounting, and usable proof. It does not repeat the completed manual review.

**Dependencies and parked triggers**

- **Infrastructure Reliability:** supplies the concrete core-operation boundary and owns credential, execution, recovery, scheduling, and advancement repairs. Intelligence supplies measurement. The [Reliability proposal](/tmp/infra-reliability-child.log) correctly distinguishes truthful errors from usable availability.
- **Architecture Minimalism:** Gate 2 must settle the `loopflow.wave-agents` direction. Capture causal intent, operation, Work, Run, Home, and provider attempt without requiring either permanent generic Work controllers or Task-only durable execution. Do not recreate old controllers to make evidence collection convenient.
- **Product:** owns external adoption, Desktop presentation, and open-Session organization. Intelligence supplies supported inspection data and context diagnosis; it does not acquire a broad dashboard remit.
- **Human-selected external work:** supplies real use and prompting cases. Evidence does not choose which portfolio Project deserves execution.

Park broad replay until a named investigation needs it; optimization until comparable usage/delivery/gate evidence supports a decision; automated monitoring and pre-human recovery claims until a recurring failure justifies them; and new context budgets until measured pressure identifies a concrete loss or cost. A parked trigger opens a scoped proposal, not automatic execution.

**Challenges and questions returned to the parent**

1. **Ratify the strict context rule.** Explicit missingness must not make absent required Loopflow context count as successful reconstruction. Conversely, unavailable vendor cost need not prevent an otherwise complete context explanation.
2. **Ratify Reliability measurement inside this single bet.** The fourteen-day population is acceptance evidence, with runtime repair staying in Infrastructure. It is not a second monitoring Project.
3. **Resolve the architecture choice at Gate 2.** The evidence contract can preserve causal identities across either design; implementation should follow the accepted execution boundary.
4. **Confirm capacity and the external sample.** The proposal requires one active Intelligence Task and real usage from at least two named external repositories. Insufficient use or late instrumentation remains an unmet proof requirement, not a reason to manufacture activity.

To complete fourteen consecutive days by the proposed review, coverage must begin no later than **2026-10-07T00:09:59.226362+00:00**. If it starts later, report the observed interval and keep the KR unchecked.
