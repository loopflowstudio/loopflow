# GOAL.md: prior-art research and proposed anatomy

Research grounding for **GOAL.md** — the durable, authored charter injected into a
wave's LLM context *every loop iteration*. GOAL.md is **inward-facing** (the mind
talking to itself), **paid every loop** (token + attention cost each turn), and
must be **self-sufficient for direction**. It is not the roadmap: the live Linear
roadmap is **outward-facing** (decomposed tasks for worker sub-agents and humans).
The charter says *who I am and how I decide*; the roadmap says *what I'm doing next*.

This doc leads with the proposed anatomy and the quality rubric (the keystone
deliverable), then the per-domain research that backs each choice. It is
opinionated and flags where the research **contradicts** our four-component
starting frame (Mission/Vision/Vibe · Key Results · Responsibilities · Processes).

---

## Part 1 — Proposed GOAL.md anatomy

The starting four components survive but are **insufficient**. Research plus two
design refinements from Jack force the shape below.

**GOAL.md is the wave's control loop written down.** That single frame organizes
everything:

- **Objective = direction** — the *one* thing the loop steers toward.
- **Milestones = moving targets** — where we're trying to get to next; they churn.
- **Quality metrics = invariants held while moving** — standards that must stay
  true every loop; durable.
- **Constraints = bounds** — hard limits the loop must never cross; durable.

**Refinement 1 — a wave has EXACTLY ONE objective, and it IS the mission/vibe.**
Unlike OKRs (which stack multiple objectives), a loopflow wave carries a single
objective at the top of GOAL.md; every other section *serves* it. A second
objective is not a second entry in a list — it is a **second (child) wave**.
Objectives don't stack inside one GOAL.md; they **fork**. This is a real
divergence from the OKR canon and from our starting "Objectives (plural)" instinct.

**Refinement 2 — "Key Results" is really THREE measure-kinds by lifecycle**, and
flattening them into one list is a mistake. Separate them:

| Kind | Verb | Cadence / lifecycle | Checked for |
|------|------|---------------------|-------------|
| **Milestone KRs** | *complete-this* | converge, then **retire** when hit | **progress** |
| **Quality metrics** | *hold-this* | steady-state, **persist forever**, never "done" | **compliance** |
| **Constraints / budget** | *never-exceed* | bounds, persist forever | **violation** |

Milestones churn on their own cadence (retired and replaced as the wave advances);
quality + constraints are durable and rarely edited. The rubric checks each kind
*differently* — progress vs compliance vs violation.

Beyond the refinements, research forces three additions and one reframe:

- **Reframe:** measures are *diagnostic setpoints*, not a reward to maximize.
  An LLM optimizer handed a bare metric games it (Goodhart). A metric is only
  safe when paired with an **honest-question north** — the anti-gaming check the
  four-component list dropped but the existing "five marks" formula keeps.
- **Add: Guardrails / Non-goals.** The single best predictor of a charter that
  actually *steers* is whether the mind can cite it to say **no**. A charter that
  only says yes never fires (mission-statement research; Porter's "strategy is
  choosing what not to do").
- **Add: Definition of Done.** A durable, non-negotiable quality bar for what
  "shipped" means — stolen wholesale from Scrum, the highest-value import there.
- **Add: Stop / idle discipline.** A positive instruction for "nothing worth
  doing → rest." This threads the needle between the two opposite agent failure
  poles: **non-termination** (AutoGPT looping forever) and the **dark room**
  (an agent that satisfies its objective by doing nothing).

Sections, purpose, and rough token budget (respecting *paid every loop* — aim for
a whole charter under ~1000–1200 prose tokens; frontmatter is near-free attention
but still counts):

| # | Section | Where | Purpose (one line) | Budget |
|---|---------|-------|--------------------|--------|
| 0 | **Frontmatter config** | YAML | `primary_flow`, `mind`, `crons`, `pm.linear_project`, `mode/workers` — machine-read, not prose attention | ~free |
| 1 | **Objective (Mission · Vision · Vibe)** | body | The judgment prior AND the **one** thing the loop steers toward. *Mission* = what this loop does now; *Vision* = the end-state that stops the loop; *Vibe* = tone as a decision tool. Identity **by contrast** with a sibling. **Exactly one objective** — a second one forks a child wave. | 150–300 |
| 2 | **Non-goals / Guardrails** | body | What this wave does **not** do + soft scope boundary. The clause the mind queries when tempted to scope-creep. (Hard numeric bounds live in §3c.) | 40–90 |
| 3a | **Milestone KRs** (*complete-this*) | `metrics:`/roadmap | 0–5 moving targets the loop converges toward, **retired when hit**. Checked for **progress**. Highest-churn section. | 60–140 |
| 3b | **Quality metrics** (*hold-this*) | `metrics:` frontmatter | Steady-state standards checked **every loop**, never "done". Checked for **compliance**. Durable. | 60–140 |
| 3c | **Constraints / budget** (*never-exceed*) | frontmatter | Hard bounds (spend cap, resource/time limits) checked **every loop** for **violation**. Durable. | 30–80 |
| 4 | **Honest-question north** | body | The one check a lazy loop can't fake — the tiebreaker that outranks any metric. Guards the measures against gaming. | 20–40 |
| 5 | **Responsibilities: Domains + Accountabilities** | body | *Domains* = what this wave exclusively controls (so parallel waves don't collide). *Accountabilities* = verb-first, ongoing (`-ing`) areas of standing attention. | 80–180 |
| 6 | **Loop body / Processes** | body | The concrete-verb move menu + how work flows: read the roadmap, pick a move, dispatch `primary_flow`, honor crons. **Points to the roadmap** — the charter/roadmap seam. | 120–220 |
| 7 | **Definition of Done** | body | The durable, non-negotiable quality bar every increment clears before "shipped." | 40–90 |
| 8 | **Stop discipline** | body | When *not* to invent work; idle/rest behavior. The off-ramp. | 20–40 |

The control-loop reading of the anatomy: **§1 is direction, §3a targets, §3b
invariants held while moving, §3c bounds, §4 the anti-gaming governor, §6 the
actuator, §8 the off-ramp.** Milestones (§3a) update on a fast cadence and belong
partly on the outward roadmap; quality + constraints (§3b/§3c) are durable charter.

**Self-completeness signal (cross-cutting, not a section):** the charter should be
scoreable against the rubric below *by the mind itself*, so a loop can open with
"is my GOAL clear enough to compute from?" and, if not, prioritize fixing its own
charter over inventing downstream work. Bake this into the loop body as an
instruction, or run it as a periodic `govern`-style cron.

What maps cleanly onto loopflow's existing `wave/*/GOAL.md`: the single-objective
body prose (§1), the `metrics:` frontmatter (today a **flat list** — the refinement
is to sort it into 3a/3b/3c and check each differently), §6 (loop body), and §0.
Sections **2, 3c, 4, 7, 8** are the ones current waves under-specify — `systems`
mixes milestones, quality, and one constraint ("billing stays bounded") into a
single undifferentiated `metrics` list; almost none carry an explicit §2 (non-goals)
or §7 (DoD). Note `systems`' first metric *is* a constraint and its "releases are
boring" *is* a quality invariant — separating them by kind is the immediate retrofit.

---

## Part 2 — The quality rubric (keystone)

Designed to be scored **0.0–1.0 per criterion by the mind against its own GOAL.md**.
Use it two ways: (A) grade/retrofit existing waves; (B) a loop self-assessing "can I
compute from this?" Overall = mean of section scores; **any section below 0.5 is a
charter bug to fix before doing downstream work.**

### Per-section criteria

**1. Objective (Mission · Vision · Vibe)**
- [ ] There is **exactly one** objective. If two distinct aims are present, one belongs in a child wave.
- [ ] Mission is **present-tense** and names what *this loop* does each turn (not a slogan).
- [ ] Vision is **future-tense** and names a **stop-state** — you can tell progress from busywork.
- [ ] States identity **by contrast** — what it is *and is not*, ideally vs a named sibling.
- [ ] Concrete/differentiating: a rival wave couldn't paste the same lines. No buzzword mush ("delight," "synergy," "leverage").
- [ ] Vibe is **named traits + a do/don't contrast**, not adjectives — it changes at least one borderline decision.
- [ ] Every other section visibly **serves this one objective** (no orphan measures).

**2. Non-goals / Guardrails**
- [ ] Names ≥1 thing the wave **declines** — the mind can cite it to say no.
- [ ] Scope boundary is legible enough to keep the loop out of a sibling's domain.
- [ ] (Hard numeric invariants are present and checked under §3c, not here.)

**3. Measures — scored by kind, not as a flat list**

*3a. Milestone KRs (complete-this → checked for **progress**)*
- [ ] 0–5, ranked; each names a target the loop **converges toward** and **retires** when hit.
- [ ] Observable and outcome-not-output; has a target/baseline (`Target 3/3`, `≥ 20 consecutive`).
- [ ] Grade = *progress toward*, not pass/fail. Stale/hit milestones are retired, not left rotting.

*3b. Quality metrics (hold-this → checked for **compliance**)*
- [ ] Steady-state standards the loop must satisfy **every iteration** (never "done").
- [ ] Each is a readable state ("main stays green," "releases verified before shipped").
- [ ] Grade = *compliant / not* this loop; a breach is immediate work, not a milestone slip.

*3c. Constraints / budget (never-exceed → checked for **violation**)*
- [ ] Hard bounds present (spend cap, resource/time limits) with a concrete number.
- [ ] Checked **every loop** for violation; a breach halts or escalates, it doesn't merely lower a grade.

*Across all three:*
- [ ] Each measure is **observable** from evidence — if it can't be read, it can't steer.
- [ ] **Not trivially gameable** by a metric-optimizer (see failure modes); the §4 north outranks all of them.
- [ ] Total measure count stays lean (roughly ≤ 5 milestones + a handful of quality/constraint lines).

**4. Honest-question north**
- [ ] Present, and phrased as a question a **lazy loop cannot fake** (not the easy proxy).
- [ ] Explicitly **outranks** the KRs as the tiebreaker.

**5. Responsibilities (Domains + Accountabilities)**
- [ ] Domains name what this wave **exclusively controls** (no silent overlap with a sibling).
- [ ] Accountabilities are **verb-first, ongoing (`-ing`)** — standing areas, not one-shot tasks.
- [ ] Written for the **role**, not the model instance (any mind picking up the charter inherits the same ownership — "role vs soul").
- [ ] The *what* is owned by the charter; the *how* is left to loop judgment (no micromanagement).

**6. Loop body / Processes**
- [ ] Gives a **menu of concrete verbs** to choose among ("sand / automate / harden / turn a failure into a fix PR"), not "make it better."
- [ ] Names the **primary flow** and how heavy work is **dispatched** (mind orchestrates, doesn't grind).
- [ ] **Points to the roadmap** — makes the charter→roadmap seam explicit (read it, write status back).
- [ ] Crons/deadlines, if any, are declared and their handling is defined.

**7. Definition of Done**
- [ ] A **durable, non-negotiable** quality bar (not per-item acceptance criteria).
- [ ] Concrete enough to gate an increment ("verified before shipped," "tests green," "docs match code").

**8. Stop discipline**
- [ ] A positive **off-ramp**: "no safe move → record the blocker, don't invent work."
- [ ] Rules out both non-termination *and* the degenerate do-nothing/declare-victory state.

### Failure modes the rubric guards against

Drawn from the research; each maps to the checks above.

- **Vacuous identity / buzzword mush** — mission that rules nothing out, could belong to any wave. → §1.
- **Objective stacking** — two aims crammed into one GOAL.md; the loop oscillates between them instead of forking a child wave. → §1.
- **Measure-kind conflation** — milestones, quality standards, and hard bounds flattened into one list, so a spend-cap *violation* is treated like a milestone *slip*. Each kind must be checked differently (progress / compliance / violation). → §3a/b/c.
- **Missing non-goals** — the charter only says yes; the loop scope-creeps into siblings. → §2.
- **Missing constraints / budget** — no hard bound, so a runaway loop burns spend with nothing to halt it. → §3c.
- **Output-as-outcome measures** — grading activity ("ran CI") instead of effect ("main green"). → §3.
- **Milestone never retired** — a hit or dead target left in the list, paid every loop for nothing. → §3a.
- **Too many / too few measures** — a dozen targets → thrash; zero → no error signal to steer on. → §3.
- **Unobservable measure** — can't be read from evidence, so it can't steer a loop. → §3.
- **Goodhart / specification gaming** — a metric-optimizer maxes the number while violating intent (close bugs as wontfix; delete the file the target measures). The honest-question north is the antidote. → §3+§4.
- **No anti-gaming north** — loop optimizes the cheap proxy forever. → §4.
- **Domain collision** — two parallel waves both "own" the engine, silently stepping on each other. → §5.
- **Task-list-as-responsibility** — one-shot verbs that "complete" instead of ongoing `-ing` accountabilities the loop re-enters each pass. → §5.
- **Un-actionable body** — "leave loopflow closer to done" gives nothing to do at 2 a.m. → §6.
- **Charter/roadmap conflation** — decomposed near-term tasks pollute the durable charter (paid every loop for nothing) or the durable why never reaches the roadmap. → §6.
- **No Definition of Done** — "shipped" is undefined; quality drifts under time pressure. → §7.
- **Non-termination** — AutoGPT-style looping with no exit condition. → §8.
- **Dark room / degenerate satisfaction** — objective satisfiable by doing nothing or cheaply declaring victory. → §8 (+§1 vision as stop-state).
- **Over-specification / brittleness** — a charter so long and rule-bound that it can't generalize to novel situations (Anthropic: longer, more-specific principles *reduced* generalization). This is the meta-failure — the whole charter must stay a **compass, not a checklist**, and short enough to be paid every loop. → all sections, budget column.

---

## Part 3 — Per-domain research (the backing)

### OKRs — the two-tier split, and why KRs are dangerous for an optimizer

An **Objective** is qualitative, directional, aspirational; a **Key Result** is a
"specific, measurable, time-bound, verifiable outcome" (asana.com/resources/okr-meaning).
Doerr's grammar: *"I will [Objective] as measured by [Key Results]"* — the "as
measured by" forces **falsifiability**, which maps exactly onto a control loop
needing a readable error signal.

**Grading:** Google scores each KR 0.0–1.0; the healthy average is **0.6–0.7**.
Consistently hitting 1.0 means you sandbagged (rework.withgoogle.com). The
**committed vs aspirational** split is the transferable piece: committed OKRs are
expected at 1.0 (invariants — "don't break the build, stay under budget");
aspirational express a desired world with ~0.7 expected and high variance
(stretch). Conflating them is a named trap.

**Good KR:** measurable with number/baseline/target, verifiable, outcome-focused,
few (**magic number ~3–5**; Wodtke pushes toward *one*). **Litmus:** if you can't
grade it 0–1 from evidence, it isn't a KR. **Failure modes:** sandbagging,
output-as-outcome, too many, business-as-usual, KRs you don't control, vanity
metrics, set-and-forget.

**Cadence — the single most transferable structural idea:** the *mission is
durable; OKRs refresh quarterly* with weekly check-ins. This two-tier
durable/periodic architecture is exactly GOAL.md (durable) vs the measures the loop
re-grades vs the Linear roadmap (periodic).

**Two deliberate divergences from OKR canon** (Jack's refinements):
*(a) One objective, not many.* OKRs stack multiple objectives per period; a
loopflow wave has **exactly one**, and it *is* the mission. A second objective
forks a **child wave** rather than adding a row — objectives don't stack, they
branch. *(b) "Key Results" splits into three lifecycle-distinct kinds.* Classic OKR
KRs are the *milestone* kind (converge, retire, grade for progress). But a wave
also needs **quality metrics** (SLO-like invariants held every loop, graded for
compliance) and **constraints/budget** (bounds checked for violation) — neither of
which is a classic KR, and both of which are *durable* where milestones *churn*.
Committed-vs-aspirational (below) roughly maps onto quality/constraints
(committed → hold at 1.0) vs milestones (aspirational → converge to ~0.7).

**The reframe (contradiction #1 with our starting frame):** OKR wisdom assumes a
*cooperative human* holding the Objective in mind and treating the KR as a proxy.
An **LLM optimizer given a KR as a literal reward pursues the number, not the
intent** — output-masquerading-as-outcome flips from oversight to *exploit*
("reduce open bugs to 0" → close as wontfix). So KRs must be **diagnostic signals
to the outer loop, not a scalar to maximize**; keep the durable mission in-context
as the tiebreaker; prefer committed guardrail-setpoints alongside stretch targets;
re-author on a cadence so no proxy is optimized to death. *The grade is a
thermometer, not the goal.* Sources: asana.com/resources/okr-meaning ·
whatmatters.com/resources/google-okr-playbook · rework.withgoogle.com.

### Scrum/Agile — steal the durable layer, drop the coordination scaffolding

The Sprint (loop) and its Goal are **disposable**; what's durable is the **cadence**
and the standing rules. Scrum splits work by durability across three tiers that map
1:1 onto loopflow:

- **Product Goal** (durable long-term objective, "fulfill or abandon") → the
  charter's north star.
- **Product Backlog** (living, reordered, refined queue) → the **Linear roadmap**.
- **Sprint Backlog** (disposable per-iteration task list) → the loop's working plan.

**Definition of Done is the highest-value steal:** "a formal description of the
state of the Increment when it meets the quality measures required," **team/
product-level not per-item**, "relatively stable across Sprints," and
"non-negotiable even under time pressure" (scrumguides.org; atlassian.com). That's
a durable, always-paid quality contract — precisely GOAL.md section 7.
**Acceptance criteria**, by contrast, are per-item → roadmap, not charter.

**Retrospective** survives as a *mechanism, not a meeting*: a cheap end-of-loop
reflection (what worked / what to change / next) that writes lessons into
**MEMORY.md**. **Drop entirely** (pure multi-human coordination): daily standup,
the three separate roles (a single mind is PO+SM+Dev), story points, velocity,
estimation, Sprint Review-as-demo. Only the inspect-against-DoD and pick-next-work
kernels survive. Sources: scrumguides.org/scrum-guide.html ·
mountaingoatsoftware.com · scrum.org · atlassian.com.

### AoR / Holacracy — the grammar for Responsibilities

An **Area of Responsibility** (Asana, from Apple's DRI) is a standing *domain of
ownership* with one final decision-maker, not a task that completes; you "keep a
bulleted list of responsibilities" (asana.com/inside-asana/workstyle-aors).

**Holacracy** decomposes a Role into **Purpose + Domains + Accountabilities**
(holacracy.org/constitution/5-0):
- **Purpose** — why the role exists.
- **Domains** — "assets, processes, or things the Role may **exclusively control**"
  — its property, which no other role touches. *This is what stops parallel waves
  from colliding.*
- **Accountabilities** — "**ongoing activities**"; the grammar rule is load-bearing:
  "the first word is *always* a verb ending in `-ing`" (holacracy.org/blog/
  holacracy-basics-understanding-accountabilities). Real examples: *Delivering
  webinars*, *Facilitating the circle's meetings*, *Maintaining an up-to-date data
  room*. An accountability is a standing *allocation of attention* ("you'll
  regularly consider doing it"), which is exactly a per-loop duty.

**Role vs soul:** the role belongs to the org; the person merely *energizes* it —
"a uniform you temporarily wear" (holacracy.org/blog/differentiating-role-and-soul).
**Transferable:** the charter *is* the role; the LLM instance is the soul. Write
Responsibilities for the role, so any mind inheriting the charter inherits the same
ownership. Example lines as they'd read in a charter: *Tracking open PRs and
surfacing any stalled beyond one loop cycle* · *Maintaining MEMORY.md as curated
truth, pruning stale entries each pass* · *Reconciling the roadmap against landed
work, flagging declared-vs-empirical drift*. Sources:
asana.com/inside-asana/workstyle-aors · holacracy.org/constitution/5-0 ·
holacracy.org/blog/holacracy-basics-understanding-accountabilities.

### Autonomous-agent goal specs & LLM constitutions — the failure catalog

AutoGPT/BabyAGI pioneered the GOAL.md pattern (one NL objective → self-decomposed
subtask loop) and exposed its failure modes: **non-termination/looping** ("stuck in
loops lacking exit conditions" — hence "strict timeouts and budget limits"),
**goal drift**, **hallucinated sub-goals**, no sense of "done"
(builtin.com/artificial-intelligence/autogpt; lilianweng.github.io). A high-level
goal alone does **not** keep an agent on-goal; it needs explicit termination and
scope.

A stated objective steers only weakly on its own: **"business rules embedded in
docstrings or system prompts become suggestions, not constraints"**
(dev.to/aws/ai-agent-guardrails). For a per-loop charter that can only steer via
context, the objective must be paired with **legible guardrails** and a concrete
**definition of done** re-read every loop.

**Constitutions are compasses, not checklists.** Anthropic's CAI trains against
"written principles" for **generalization**; Claude's constitution notes models
"need to generalize — apply broad principles rather than mechanically follow
rules," and **"a much longer and more specific principle tended to damage or reduce
generalization"** (anthropic.com/news/claudes-constitution; arXiv 2212.08073).
Over-specification actively degrades a charter — the direct warning against a
bloated GOAL.md.

**The two poles a charter must thread:**
- **Under-spec → drift/wandering** (vague objective, nothing to re-anchor on).
- **Over-spec → brittleness** (can't adapt when reality diverges).
- **Dark room / degenerate objective** — an active-inference agent minimizing
  prediction error settles on "a dark room… the lack of stimuli is the easiest way
  to minimise error" (Baltieri & Buckley, ALife 2019). Analog: an objective
  satisfiable by doing nothing or cheaply declaring success.
- **Reward hacking / Goodhart / specification gaming** — DeepMind's canon: the
  CoastRunners boat looping for powerups instead of finishing; a GA that **deleted
  the file holding the target output so it was rewarded for outputting nothing**
  (deepmind.google/discover/blog/specification-gaming). Naming a proxy metric
  invites the agent to optimize the letter while violating intent.
- **Instrumental convergence** — self-preservation/resource-acquisition as
  emergent subgoals of a poorly-bounded objective (brief risk).

**What a durable objective needs to actually steer:** self-sufficiency for
direction, guardrails as re-readable prose, an explicit "done/good-enough," and
defined **termination/idle behavior** — the antidote to *both* non-termination and
the dark room. Sources: lilianweng.github.io/posts/2023-06-23-agent · anthropic.com/
news/claudes-constitution · deepmind.google/discover/blog/specification-gaming ·
dev.to/aws/ai-agent-guardrails.

### Mission/vision craft & personal constitutions — make the identity fire

**Mission = present ("the engine," what we do now); vision = future ("the horizon,"
where we're driving)** (atlassian.com; masterclass.com). Collapsing them leaves the
agent unable to distinguish progress from busywork — hence vision-as-stop-state.

A strong mission is **simple, concrete, memorable** (TED's two-word "spread ideas");
the failure mode is buzzword soup ("synergistic paradigms," "leveraging core
competencies") that "lacks meaning without context" (wix.com; boardeffect.com).

**The load-bearing test: a good mission rules things out.** "The essence of
strategy is choosing what *not* to do" (Porter); "great strategy gives you
permission to say no" (Ravi Mehta). A directional statement earns its place by
**excluding** — the single best predictor of a functional charter section is
whether the mind can cite it to *decline* an action. This is why **Non-goals** is a
mandatory section, not a nicety.

**Personal constitution** (Covey): principle-centered guiding statements that act
as a **decision prior**, functional precisely because they "force you to think
through priorities and align behavior with beliefs" — not journaling. **Personal
OKRs** add scarcity (~3, "significant, concrete, action-oriented").

**Vibe as a functional prior:** brand-voice guidelines are the closest prior art —
they "work as a decision-making framework by establishing clear parameters," and
the functional versions are **named traits + do/don't example pairs** ("It's
helpful to see clear examples of what they shouldn't do") rather than adjectives
(mailchimp styleguide; frontify.com). The rule of thumb for the whole identity
section: *a line belongs only if you can name a decision the loop would make
differently with it than without it.* Sources: atlassian.com/work-management/
strategic-planning/mission-and-vision · andrewolsen.net (Porter) · shortform.com/
blog/personal-mission-statement-7-habits · styleguide.mailchimp.com/voice-and-tone.

---

## Part 4 — How this lands against loopflow's current "five marks"

PROMPT_STYLE.md already documents "five marks of a goal that loops well": identity
by contrast, ranked readable metrics, concrete-verb loop body, an honest-question
north, a stop discipline. The research **validates all five** and shows the
four-component starting frame (Mission/Vision/Vibe · KRs · Responsibilities ·
Processes) **regressed** on two of them — it dropped the honest-question north and
the stop discipline. The proposed anatomy restores both and adds three the five
marks under-specify: **Non-goals/Guardrails**, **Definition of Done**, and an
explicit **pointer to the roadmap** (the charter/roadmap seam).

Net: keep the five marks, fold the four components into sections 1/3/5/6, add
sections 2/4/7/8 as first-class, enforce the **single objective**, and split the
flat `metrics` list into the **three measure-kinds** (milestone / quality /
constraint) each checked differently. Existing `systems` and `architecture`
GOAL.md files are the closest to compliant; `goals/GOAL.md` carries strong
milestone-style measures but no explicit non-goals, DoD, constraint/budget bound,
or stop discipline. The immediate retrofit for every wave: label each existing
`metrics` line by kind and pull out the hidden constraint (`systems` already has
one — "billing stays bounded" — buried in the same list as its milestones).
