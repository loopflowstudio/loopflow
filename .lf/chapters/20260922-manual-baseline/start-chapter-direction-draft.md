# Next chapter direction — provisional

**Proposed interval:** 22 September–20 October 2026  
**Status:** Direction proposal only. No Wave, Project, KR, Task, or Work state has
been changed. This Run is not Task-bound. The technical scopes below are a
dependency map, not yet the accepted Project portfolio.

## Briefing from the closing chapter

- The reviewed portfolio contained 39 current KRs: 2 held, 16 did not hold, and
  21 were unknown.
- The chapter's main technical result was the execution redesign: remote clients
  stopped treating `lfd` as the runtime/data API and moved toward complete `lf`
  execution on the owning Home, with SSH as explicit transport.
- Failures encountered while dogfooding Product correctly drove much of the
  semantic runtime redesign. That work clarified durable Infrastructure
  responsibilities; it was not development happening in the wrong place.
  Product surface outcomes were nevertheless displaced by the depth of the
  execution repair.
- Intelligence adapted launch-contract and replay designs, but the operating
  trace did not catch up: all Trace KRs failed; real-ledger readers, scheduled
  telemetry, coverage, and replay evidence remain unreliable or absent.
- Infrastructure architecture legibility held for the observed landing window,
  but unattended operation did not: credentials stalled Project controllers,
  scheduled telemetry failed, and broad performance/resource goals were not
  active priorities.
- Current PM work is much smaller than historical Work counts suggest:
  Infrastructure has one open PM Task, Intelligence none, and Product six.
  `engbot` has no repository charter or Project.
- The same Linear OAuth failure stopped all three Project controllers after
  28 August. This is a current blocker, not historical color.

## Chapter posture

### Gate 1 human direction — in conversation, not yet accepted

The human considers the summer chapter successful as a strategic reset. Spring
development had moved too quickly: design lagged implementation, product scope
expanded toward competitor features, and the product was being sold ahead of
its actual maturity. The summer recovered a distinctly Loopflow design instead
of maximizing feature count, and adapted that design to the lived reality of
models that are highly capable but imperfect.

The cost was process bloat and excessive parallel work. The next chapter should
carry fewer simultaneous commitments and leave real capacity for non-Loopflow
work, rather than making Loopflow's own operating machinery consume the whole
portfolio. This is a planning constraint and an experience goal, not merely an
efficiency metric.

The strongest current Product tension is Desktop value. The human still works
mostly through the terminal and does not yet receive meaningful day-to-day value
from the Desktop app. This is disappointing precisely because the design now
feels close rather than misguided: extended use of the Sessions pane has made
its interaction model credible and likely sufficient. The next Product bet
should build on that validated design and connect it to a useful operating
experience, rather than restarting the Sessions design.

Three capabilities would make that Desktop experience valuable:

1. A UI centered on Projects, KRs, and Tasks that can absorb new ideas and
   iteration, supporting a durable way of working rather than a sequence of
   short-lived terminal interactions.
2. The most reliable place to track and organize every open model Session, with
   no major interaction disadvantage relative to running models in Warp or
   native Ghostty.
3. Good provider-account juggling plus trustworthy evidence and tools for
   exploring how prompting can improve.

Together these suggest one user loop: organize durable intent, conduct the live
model work, then learn from the resulting evidence. The open planning question
is whether that loop can be delivered as one focused Product bet or must be
sequenced across Product, Infrastructure, and Intelligence without recreating
the excessive parallelism of the prior chapter.

None of those component capabilities is independently exciting to the human.
The outcome they should unlock is portfolio-level continuity: at least three of
Cube, Etude, Kata, and Hootro are genuinely in flight each week. Loopflow should
make this external portfolio sustainable without its plans and Sessions
collapsing into each other, without repeatedly reconstructing context, and
without Loopflow's own process consuming the attention that should go to them.

“In flight” requires substance. In a normal week, each project should either
ship something meaningful or show active execution toward one explicit
priority. Preserved context, an organized backlog, or an open Session alone does
not count as progress. Cube, Etude, Kata, and Hootro are external work enabled
by Loopflow; Loopflow's own internal movement is valuable only insofar as the
product helps sustain that broader portfolio.

The active set may rotate freely. A week succeeds when any three of the four
meet the “in flight” standard; the fourth may be intentionally quiet without
creating a failure or a standing obligation to catch up.

The human remains the primary driver. Loopflow should make the portfolio
legible, preserve context, organize Sessions, and make choices easy, but it
should not autonomously decide which project or priority matters each week.

Depth outranks breadth within each project, while project-level parallelism is
desirable. Cube, Etude, Kata, and Hootro may advance concurrently, but each
active project should concentrate on one priority done excellently rather than
fragmenting into many simultaneous efforts. Within Loopflow, planning,
Sessions, account juggling, and prompting evidence should compose into the
support for that operating model rather than becoming disconnected feature
threads.

Product dogfooding should therefore focus outside Loopflow: use Cube, Etude,
Kata, and Hootro as the real proving ground for whether the product sustains
several projects with one clear priority each. Failures found while Loopflow is
used to build and operate Loopflow belong under Infrastructure reliability (or
Intelligence when the failed contract is evidence), not in the Product
dogfooding Project. “Dogfooding” must not mix product value with self-hosting
repair.

Each external project should expose a well-defined, explicit active priority
through existing planning objects. A KR can be active when several Tasks serve
one outcome; a Task can be active when one concrete change is the focus. This
should remain a selection over the existing Wave → Project → KR/Task model, not
a fourth planning noun or a priority system inferred from whichever Session is
currently alive.

That focus must not become a cosmetic Desktop pin over stale planning. The
durable Work of record in each external project must be kept current with what
the human actually wants to pursue next: Projects and KRs express the current
bet, Tasks express the concrete work, and obsolete or displaced intent is
explicitly dispositioned. Every surface and subsequent agent should receive the
same current direction from that record.

For at least three of the four external projects in each week, that current
Work record must be paired with forward momentum on at least one Task in service
of the selected focus. Planning freshness without execution does not count, and
an open Session without current Work direction does not count.

Parallelism belongs across the portfolio, not deep inside each project. Each
project may retain a broad backlog, but its active execution frontier should
remain deliberately small and legible. For chapter judgment, call one or two
active Tasks **Small**, two to four **Medium**, and two to eight **Big**. Cube,
Etude, and Hootro are expected to read as Small; Kata as Medium; Loopflow as Big
across its Projects. These labels may be encoded in a company-specific planning
schema so chapter planning and review can judge portfolio load consistently.
They are not new Work kinds or generic lifecycle constraints, and core Loopflow
should not block execution or manufacture warnings from them.

The closing review answers **where the system is technically**. Start-chapter
must now choose **which user changes matter next**. The technical map explains
how to reach those outcomes; it does not choose them.

Plan in this order:

1. Choose a small set of concrete changes in what a user can understand, do, or
   trust through Loopflow.
2. Define Product Projects and KRs around those observable changes.
3. Trace each Product outcome through the current execution and evidence map.
   Promote only the blocking Infrastructure and Intelligence gaps into chapter
   Projects or KRs.
4. Dogfood the intended path continuously. Route each failure to Product,
   Infrastructure, or Intelligence according to the contract that failed.

This continues the development loop that drove the execution redesign, while
shifting the chapter's center of gravity back to user behavior.

The three Waves do not start from the same source:

| Wave | Primary planning source |
| --- | --- |
| Infrastructure | The closing review's observed execution failures, durable architectural remainder, and the dependencies of accepted Product outcomes. |
| Intelligence | The closing review's missing or unreliable evidence edges, plus the proof required by accepted Product outcomes. |
| Product | The human's hidden backlog of desired user changes and current dogfood pain. Prior KRs and open Tasks are context, not a substitute for choosing the product. |

Infrastructure and Intelligence can therefore be bootstrapped substantially
from this review. Product requires a direction conversation before its Projects
and KRs are credible. Classifying an existing Task as Product does not by itself
mean it should carry into the next chapter.

## Wiping the slate clean

Starting from user priorities must not silently discard the architectural tail.
The accepted chapter packet needs a complete disposition ledger for the prior
portfolio and every known material debt. Each item gets exactly one treatment:

- **Carry:** required to deliver or prove an accepted user outcome. Name its
  Wave, Project, and owning Task.
- **Invariant:** behavior that must keep holding, but is better enforced by a
  checker or cadence than funded as a new Project.
- **Park:** real debt outside this chapter. Preserve the evidence, consequence,
  owner, and an objective trigger for reconsidering it.
- **Retire:** an obsolete design, duplicate Work object, or aspiration we no
  longer intend to satisfy. Remove it from the live planning surface after the
  plan is accepted.

An `unknown` KR is not automatically debt. If its truth would change the next
plan, create one bounded fact-finding Task. Otherwise retire the unsupported
claim instead of carrying ambiguity forward.

The slate is clean only when:

1. every prior Wave, Project, KR, current open Task, stale historical Work
   record, and material review finding appears once in the disposition ledger;
2. every carried item belongs to one accepted Project and every parked item has
   a reactivation trigger;
3. live PM and Work state are reconciled to the accepted ledger, with no orphan
   Project, duplicate Wave, or historical Task count masquerading as current
   work; and
4. the archived review and start packet preserve the discarded context, so the
   live plan can be small without making the history disappear.

### Preliminary debt ledger

| Known remainder | Proposed treatment before user priorities are finalized |
| --- | --- |
| Linear OAuth expiry stops Project controllers | Carry: Infrastructure / Reliability, under the dogfood-availability KR (`LOO-279`). |
| Task worktrees can contaminate or depend on canonical main | Carry: Infrastructure / Architecture Minimalism (`LOO-257`). |
| Process, Run, Work, Git, and presentation state can disagree | Use the selected user journeys to define the required correction; retain honest `unknown` and non-authoritative telemetry as invariants. |
| Shared `lfd.db` migration ownership can still collide | Record as Infrastructure debt; carry only if it blocks an accepted journey, otherwise park with recurrence or data-loss as the trigger. |
| Settled Runs lack complete context, contracts, usage, cost, or replay proof; prior capture lost writes and produced decode failures | Select the evidence edges required by accepted user outcomes for Intelligence work; park the rest explicitly rather than claiming complete Trace. |
| Session and recovery work remains open | `LOO-251` seeds a forming Desktop Project; keep `LOO-148` unclustered until an accepted Product outcome requires it. |
| Chapter and Discord work remains open | Carry `LOO-278` and `LOO-185` together under Product / Company Dogfood. |
| `loopflow.wave-agents` contains the flow-driven Work advancement redesign but has no Linear Task or PM owner | Create a tracked Task under Infrastructure / Architecture Minimalism; preserve the current branch as implementation evidence, not as planning identity. |
| `LOO-274` — restore Mac fleet/roadmap | Retire. Human direction rejected this as a next-chapter Product priority. |
| `release-stability`, `product-performance`, and `wave-chat` historical Work no longer matches current PM scope | Retire after accepted replacements are recorded. |
| iOS, broad performance/resource targets, prompt optimization, and other unsupported prior aspirations | Retire unless the user-priority conversation deliberately selects them; do not leave them as implicit promises. |

The final ledger must expand this table to the exact prior KR and Task roster and
reconcile its counts to the closing review before any live planning mutation.

## Proposed Wave map

Keep three Waves. Do not create a new Runtime Wave: the runtime foundation is
Infrastructure, its evidence loop is Intelligence, and its user contract is
Product.

### Infrastructure

**Objective:** Home-local execution is dependable. Work starts, resumes,
recovers, and ships on the owning Home without stale authority, missing
telemetry, credential expiry, release transitions, or machine changes stranding
it or requiring hidden repair.

**Boundary:** Own execution mechanics, process capability, Home placement,
credentials, worktrees, promotion, CI/landing, releases, and scheduled
operations. Do not own product presentation or the monitoring interpretation of
those facts.

### Intelligence

**Objective:** Distributed execution explains itself. A user can reconstruct
what ran, where, under which authority and context, what happened, what it cost,
what followed, and whether the evidence is current—without monitoring becoming
a control plane.

**Boundary:** Own Run/context evidence, cross-authority trace joins, replay,
usage, coverage, monitoring, freshness, and disagreement detection. Do not own
execution decisions or UI lifecycle.

### Product

**Objective:** A user can understand, steer, recover, and review Home-local Work
without dropping to a terminal or creating a second runtime in the interface.

**Boundary:** Own the shared user contract, Mac/CLI/Discord presentation and
legal controls, Sessions interaction, and chapter planning/review. Consume
Infrastructure and Intelligence authority; do not reimplement it in Swift or a
surface-specific service.

## Candidate technical proof map

These candidate scopes describe how user outcomes could be enabled and proved.
They are not seven automatic bets. The final portfolio should be rewritten from
the accepted user priorities, with technical work retained only where it is on
their critical path.

### Minimum technical commitments from human direction

- **Infrastructure:** at least one release-focused KR, and at least one KR
  focused on documentation, simplification, and architectural minimalism.
- **Intelligence:** at least one KR focused on the reliability and usability of
  trace and context. Further Intelligence commitments remain open.

These are a floor, not the full technical portfolio. They do not imply one or
two Projects per Wave; Project boundaries should follow the coherent bets that
emerge after Product priorities and the debt ledger are reconciled.

### Infrastructure / Reliability

**Definition:** Loopflow remains usable while it is being used to build
Loopflow, and releases complete through the configured delivery path without
hidden repair.

**KR — releases:** Every scheduled release opportunity is durably accounted for
as on-time, caught up, deferred, or failed, and two consecutive opportunities
complete through the configured path as a truthful no-op or complete exact-tag
publication; at least one real publication reaches users, required verification
passes, and no manual Git, worktree, artifact, or database repair is required.

**Seed Tasks — releases:**

1. Account for and catch up every scheduled release opportunity.
   - **Why:** thirty-three launchd-mediated runs prove that the configured path
     executes, but the ledger cannot distinguish calendar firings from manual
     triggers; one run started only after wake and another day had no separate
     firing while its predecessor remained active.
   - **Success:** record every daily obligation exactly once as on-time, caught
     up after wake, deferred behind an active attempt, or missed; resume caught-up
     and deferred work automatically without duplication. Move the same contract
     to another Home or provider only if the local machine cannot satisfy it.
2. Make scheduled release outcomes truthful and successful.
   - **Why:** on 22 September the release log said “Release did not complete,”
     while its cron receipt said `succeeded`; process exit is being mistaken for
     release success.
   - **Success:** published, no-op, resumably blocked, and failed outcomes produce
     distinct truthful receipts; repair only reproduced blockers; then settle two
     consecutive opportunities without manual repair, publish at least once, and
     pass exact-tag smoke checks on every required surface.

**KR — dogfood availability:** Across 14 consecutive days of real
self-hosting, every requested core planning or execution operation reaches
provider progress or one bounded, truthful, actionable block; no credential
expiry, stale resident, or silent controller failure creates a hidden outage,
and status never claims availability that the configured path contradicts.

**Seed Tasks — dogfood availability:**

1. `LOO-279` — make Linear OAuth recovery survive unattended Project runs.
2. Measure dogfood availability from Intelligence evidence. Over 14 days,
   classify every attempted `lf` Run or Work advancement as progressed,
   intentionally waiting, failed by a named cause—including Linear auth—or
   unknown; report counts and rates, link failures to evidence and owning
   Tasks, and keep the read model non-authoritative.

### Infrastructure / Architecture Minimalism

**Definition:** Loopflow contains only the public, durable, and cross-process
concepts it needs across planning, execution, evidence, operations, releases,
and product surfaces, and presents one coherent model through code, CLI,
documentation, and checks.

**KR:** Across the repository, every public command, cross-module API, durable
data concept, wire or process contract, and control authority maps to one
intentional product concept and owner; the CLI, user docs, architecture map,
and checker agree; no obsolete path, duplicate source of truth, or unexplained
compatibility seam remains.

**Seed Tasks:**

1. `LOO-257` — base new Task worktrees on current canonical main.
2. Finish flow-driven Work advancement. Track the existing
   `/Users/jack/src/loopflow.wave-agents` branch and prove that one bounded
   operation starts or resumes dead-controller Work, survives a killed provider
   with one successor at the same Flow position, and stops explicitly at human
   or terminal boundaries.
3. Run a repository-wide reduce pass across planning, execution, evidence,
   operations, releases, CLI, Mac, and Discord. Challenge every public,
   durable, or cross-process concept and high-cost subsystem against current
   user value; cut or simplify each candidate in code, or create one exact Task
   tied to a concrete failure or user cost when it cannot safely fit the pass.
   Do not produce a catalog of retained helpers or inventory private trivia.
4. Make the reduced product tell one coherent story: land the selected
   simplifications and align CLI help, DTOs, user docs, the architecture map,
   and automated checks; remove obsolete public routes and make the checker
   prevent their return.

### Intelligence / Execution Trace

**Definition:** Every execution has a complete causal record from durable Work
through Home, Run, provider, process evidence, terminal result, Work transition,
and delivery; eligible executions can be reproduced from that record.

1. For 20/20 sampled agent launches, one drill path resolves the initiating Work
   boundary, Home and runtime artifact, immutable launch contract and exact
   context, provider attempt and native Session, terminal evidence, resulting
   Work transition, and PR or other outcome; every absent edge is explicit and
   names its source.
2. For 14 consecutive days, every new settled agent Run contains its immutable
   launch contract, exact context manifest, provider/model/account identity,
   terminal receipt, and token/cost evidence when the provider reports it; the
   coverage denominator is published and has zero unexplained omissions.
3. Ten of ten preflight-eligible Runs replay unattended into a new linked Run
   without changing their source artifacts; every ineligible Run returns a typed
   refusal rather than reconstructing from ambient state.
4. `lf doctor`, `lf runs`, `lf usage`, and `lf activity` complete against the
   long-lived migrated store for 14 consecutive days with zero schema/decode
   failure; every retained historical gap is labeled rather than silently
   repaired.

### Intelligence / Runtime Monitoring

**Definition:** One non-authoritative read layer explains current execution and
health across Homes, preserves freshness and disagreement, and turns a new red
signal into bounded action without affecting the Work it observes.

1. Twenty of twenty live, stopped, dead, stale, unavailable, and unknown cases
   across Work, Run, provider, and OS evidence receive the correct classification
   and source age; zero dead processes appear live and zero missing source appears
   healthy.
2. Fourteen consecutive scheduled monitoring runs complete against the
   long-lived store; each novel failure creates or links exactly one actionable
   Task within one cadence, and repeated evidence does not create duplicate work
   or a permanently red check.
3. From any Wave, Project, or Task, one drill-down shows Home placement, current
   Work condition, exact live-process evidence when available, latest Run and
   Session, and delivery outcome; 20 sampled rows reconcile with their focused
   source reads, including repository scope.

### Product / Desktop

**Definition:** The native desktop presents and interacts with the same Work and
provider-native Sessions as the CLI without creating another lifecycle or source
of truth.

**KR:** From one native workspace, a user can move between company planning and
open provider-native Sessions without losing repository or Work context;
planning remains shared `lf` state, Sessions retain their native lifecycle, and
the Desktop invents neither authority nor ownership to connect them.

**Seed Tasks:**

1. `LOO-251` — complete the native Sessions multiplexer and promoted Ask handoff.
2. Integrate planning and open Sessions into one Desktop workspace. A user can
   move directly between the Wave, Project, and Task planning hierarchy and
   relevant open Sessions without losing repository or Work context, while
   planning stays shared `lf` state and Session lifecycle stays provider-native.

`LOO-148` remains unclustered recovery work until the chosen Desktop experience
requires it.

### Product / Company Dogfood

**Definition:** Loopflow's own company operates every active Wave through the
same Discord, Home, Work, and evidence paths the product promises to users. Real
use exposes product, execution, and evidence failures without creating a second
transcript or private manual operating path. Product owns the dogfood loop;
findings route to the Wave that owns the failed contract.

**KR:** Each active company Wave completes one full operating loop through
intended product paths: direction enters through its company surface, delivery
and execution remain visible, and chapter review/reset supports the next
decision; zero accepted direction disappears, crosses Waves, or creates a
second transcript, and every manual fallback is classified and routed within
one cadence.

**Seed Tasks:**

1. `LOO-185` — provision Loopflow's company Discord channels.
2. `LOO-278` — add chapter reviews and interactive plan resets.
3. Run one complete company operating cycle from Discord direction through Work
   result and chapter review; route every fallback to its owning Wave.

## Current Project dispositions

| Current scope | Proposed disposition |
| --- | --- |
| Infrastructure / Stability & Security | Rewrite into Reliability, with separate release and dogfood-availability KRs. |
| Infrastructure / Technical Architecture | Complete as a chapter bet; retain the architecture checker as Infrastructure cadence/invariant. |
| Infrastructure / Performance & Efficiency | Retire. No active Task or working evidence pipeline supports the broad resource and latency frontier. |
| Infrastructure / `release-stability` historical Work | Abandon stale Work after accepted migration; relevant continuity outcomes live in Reliability. |
| Intelligence / Trace | Rewrite and split into Execution Trace and Runtime Monitoring. |
| Intelligence / Context | Retire as a separate Project. Preserve exact launch context inside Execution Trace; drop prompt-optimization, zero-config, and Discord-relevance goals for this chapter. |
| Product / Mac Surface UX | Do not carry wholesale. Use `LOO-251` as the first input to a forming Desktop Project; select the actual user outcome from the hidden backlog. |
| Product / Loopflow API | Retire as a catch-all. Move durable runtime mechanics and the flow-driven Work advancement branch to Infrastructure / Architecture Minimalism; move chapter lifecycle to Company Dogfood. |
| Product / Auditability | Retire as a separate Project. Route chapter evidence into Company Dogfood; leave other audit work unclustered until a selected user outcome requires it. |
| Product / `product-performance` and `wave-chat` historical Work | Abandon stale Work after acceptance. Company Dogfood replaces the relevant collaboration outcome without restoring the old Wave Chat Project or a second transcript. |
| Engbot | Retire empty Wave. |

## Open Task dispositions

| Task | Proposed disposition |
| --- | --- |
| LOO-279 — Linear OAuth recovery | Carry under Infrastructure / Reliability's dogfood-availability KR; it is the immediate blocker. |
| LOO-257 — canonical-main Task worktrees | Move from Loopflow API to Infrastructure / Architecture Minimalism. |
| `loopflow.wave-agents` — flow-driven Work advancement branch | No Task exists. Create one under Infrastructure / Architecture Minimalism before carrying the implementation forward. |
| LOO-274 — restore Mac fleet/roadmap | Retire; do not carry into the next chapter. |
| LOO-251 — Sessions multiplexer | Seed the forming Product / Desktop Project. |
| LOO-148 — abandoned-Task recovery DTO | Keep unclustered; do not carry until an accepted Product outcome requires it. |
| LOO-278 — chapter review/start | Carry under Product / Company Dogfood. |
| LOO-185 — provision Discord channels | Carry under Product / Company Dogfood. |

## Tensions for human direction

1. **Wave ownership:** Product dogfooding correctly drove much of last chapter's
   runtime redesign: failures in real product use exposed defects in execution,
   recovery, authority, and placement. The proposed map preserves Product as
   that discovery pressure while assigning the resulting durable execution
   mechanics to Infrastructure. This is a clarification produced by the work,
   not a claim that the work happened in the wrong Wave.
2. **Remote proof:** Infrastructure's execution Project requires one real remote
   Home. If remote operation is not a next-chapter priority, remove that KR
   rather than simulate it.
3. **Product scope:** The proposal deliberately drops iOS, broad shared-API load
   tests, and generalized product performance. Discord provisioning and real
   company use remain explicit Product work under Company Dogfood.
4. **Intelligence scope:** Context optimization is paused until the rebuilt trace
   can support it. Exact launch context remains required evidence, not a separate
   bet.
5. **Chapter duration:** A four-week chapter forces instrumentation into the
   first half so 14-day evidence windows can close. A shorter chapter requires
   replacing those windows with sample-based proof.
