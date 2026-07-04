# Free energy, active inference & neighbors → the wave agent

Source: [softmax.com/blog/inspiration](https://softmax.com/blog/inspiration) — the
reading list of Softmax Research (org building toward "organic alignment,
coherence, and open-ended learning"; team behind `metta-ai`). The company frame
is multi-agent RL and emergent cooperation; the brief below is about the *ideas*
they curate, not the company. The load-bearing tradition for us is the
Friston/free-energy → active-inference → Levin-multiscale-competency →
cybernetics cluster. Their RL-heavy sections (exploration bonuses, NCAs, open-
endedness benchmarks) are adjacent and mostly out of scope here.

## 1. The landscape

The page is organized into ~10 clusters. The ones that touch us:

**Free energy & Bayesian mechanics.** Friston, *The free-energy principle: a
unified brain theory?* ([Nature 2010](https://www.nature.com/articles/nrn2787)),
with the accessible gloss
([SSC, "God Help Us…"](https://slatestarcodex.com/2018/03/04/god-help-us-lets-try-to-understand-friston-on-free-energy/)).
The claim: any system that persists minimizes "free energy" — a tractable proxy
for surprise / prediction error, the gap between an internal generative model
and sensory input. It has exactly **two levers**: update the model to match the
world (perception/learning), or act on the world to match the model
(action/homeostasis). "When you're hungry… you grab a piece of pizza, thereby
changing your sensory input so that it conforms to the pizza predictions." Also
here: *Making the Thermodynamic Cost of Active Inference Explicit*
([Entropy 2024](https://www.mdpi.com/1099-4300/26/8/622)) and *Towards a Geometry
… for Bayesian Mechanics* ([arXiv](https://arxiv.org/abs/2204.11900)).

**Markov blankets.** The page doesn't cite the canonical paper directly but it's
load-bearing throughout: Friston et al., *The Markov blankets of life*
([J. R. Soc. Interface 2018](https://royalsocietypublishing.org/rsif/article/15/138/20170792)).
A Markov blanket statistically partitions a system into **internal** and
**external** states, plus the **blanket** — **sensory** states (world → system)
and **active** states (system → world) — that *mediate* all exchange. Internal
and external become *conditionally* independent: coupled, but only through the
blanket. Persistence = maintaining your blanket against entropy.

**Multi-scale competency (Levin).** *Technological Approach to Mind Everywhere*
(TAME, [Front. Syst. Neurosci. 2022](https://www.frontiersin.org/journals/systems-neuroscience/articles/10.3389/fnsys.2022.768201/full)).
"All known cognitive agents are collective intelligences, because we are all made
of parts." Cognition = goal-directed navigation of *some* space (metabolic,
transcriptional, anatomical, behavioral), not just 3-D behavior. Each agent has a
**cognitive light cone** — the spatial/temporal scope of states it can represent
and care about. Subunits' stress signals *bind them* into a larger self with a
bigger light cone; agency nests within agency.

**Care as driver of intelligence.** Doctor, Witkowski, Solomonova, Duane, Levin,
*Biology, Buddhism, and AI* ([Entropy 2022](https://www.mdpi.com/1099-4300/24/5/710)).
If intelligence *is* "engaged concern for problem solving," a system's ceiling is
raised by **enlarging its sphere of concern** (its light cone). Homeostatic
setpoints scale up into "care."

**No privileged level of causation.** Denis Noble, *A theory of biological
relativity* ([Interface Focus 2012](https://royalsocietypublishing.org/doi/10.1098/rsfs.2011.0067)).
Living systems are multilevel; behavior at any level depends on the levels above
and below. Downward causation (higher levels set boundary conditions on lower
ones) is *real and necessary* — there is no master level. The page pairs it with
*Causal Emergence 2.0* ([arXiv](https://arxiv.org/abs/2503.13395)) and *Biological
Relativity: No Privileged Level of Causation* on the "formation of functional
wholes."

**Cybernetics (implicit ancestor).** Not on the page by name, but it's the trunk
this all grows from — Ashby's homeostat, Beer's Viable System Model with its
**algedonic channel** (a pain/pleasure alarm bus that bypasses the management
hierarchy to reach the top). Our code already speaks this dialect:
`AttentionKind::Algedonic` (`rust/loopflow/src/lfd/types/attention.rs`) is a
verbatim Beer borrowing.

Softmax's own thesis, worth naming: **alignment is grown, not imposed** — "the
seed of care that can develop over time into full alignment," agents as nested
believers ("cells have beliefs, organisms have beliefs, tribes … have beliefs").

## 2. The dialogue already happening

The wave design converges with this tradition on several points *without having
read it* — which is the interesting part. Precise mappings, weighted:

**Wave sovereignty ≈ Markov blanket. (Strong — the analogy carries real weight.)**
The governing principle's litmus test — "does this create a center?" — is the FEP
claim restated: a thing persists by maintaining a boundary that keeps internal and
external *conditionally independent*, coupled only through mediated states. Map it
literally:
- **internal** = the mind + its journal + the wave's worktree (`<repo>.<wave>`);
- **external** = other waves, the repo, remote humans, GitHub;
- **sensory** (world → wave) = the *listener*, folding worker rows, `lf chat`
  posts, and human messages into one timeline;
- **active** (wave → world) = the `lf` exec doors (`lf q worker run`, `lf chat
  say`) and, for remote, `lfd serve` as the **access gate**.

That gate is the sharpest match: the design already insists lfd-serve "may only
notify and gate… the moment it reimplements behavior it has become a
headquarters." That is exactly a blanket state — it controls *exchange across the
boundary*, never the internal dynamics. Waves sharing no state and coordinating
only through registry-facts + pubsub *is* conditional independence via the
blanket. This one isn't poetic; it's the same structure.

**Multi-scale wave trees ≈ Levin nesting + Noble's no-privileged-level. (Strong.)**
Wave trees (§3.7: parent/child waves, children drawing on parent headroom) are
Levin's nested competency and Noble's biological relativity in one object.
"Nothing sits above the waves" is *no privileged level of causation*. The two
causal directions are already built: **downward** = parent constrains child
(budget headroom, `spend_cap`, reaping by prefix prune) — boundary conditions on
the lower level, exactly Noble's mechanism; **upward** = algedonic escalation
child→parent→human. Each wave's GOAL.md + roadmap *is* its cognitive light cone —
its declared scope of concern.

**Algedonic escalation ≈ precision-weighted prediction error routed up a
hierarchy. (Strong, and already partly built.)** Beer's algedonic channel and
FEP's "propagate only high-precision surprise upward" are the same idea; our
`AttentionKind::Algedonic` with "only root reaches the human" is the
implementation. What's FEP-shaped and *missing* is precision weighting — see §3.

**Listener/publisher ≈ message passing between blankets. (Good.)** Pubsub where
publishing to no subscriber simply drops (`lf chat` outside a wave exits 0) is
conditional-independence-preserving message passing: sovereign publishers each own
their boundary; the listener is the sensory surface that integrates them. "Correct
pubsub semantics, not degraded mode" is the blanket refusing to leak.

**Journal-fold ≈ generative-model updating. (Partial — see the critique.)** The
append-only journal with thread/state/queue as pure folds rhymes with a system
whose beliefs are a deterministic function of its evidence stream. But a journal
*records*; a generative model *predicts*. The fold is backward-looking. The
resemblance is real at the "single source of truth, everything else derived" level
and stops at prediction.

**MEMORY.md curation ≈ model compression toward "what we know." (Good.)** "The
event log is what happened; MEMORY.md is what we know," curated as "distilled facts
and constraints, not accreted turn dumps." That is compression of an evidence
stream into a compact model — the move FEP calls minimizing description length.
The Dumb Zone rule (silent degradation past ~40% context) is **bounded
rationality** made operational: it's *why* compression is mandatory, not optional.
Beer would recognize it as requisite variety running out.

**Heartbeat + event-driven wakeup ≈ active sampling. (Partial.)** The mind waking
on worker-completed / message / roadmap-change events plus a heartbeat tick is
event-driven, but the heartbeat is a *poll* ("start the next roadmap item when
quiet"), not active inference's *predictive* sampling (act to test a belief and
harvest the error). Same silhouette, different engine.

## 3. The critique — what this tradition says we're missing

**The mind reacts; it never predicts.** This is the central gap, and the design
half-names it. The mind folds worker summaries and dispatches the next thing. It
never forms an *expectation* about a worker's outcome, then acts on the
**difference** between expectation and result. In FEP terms the wave is
all-perception-no-prediction: a reactive fold, not a predictive controller. Nothing
is surprised. Concretely: `WorkerDispatched` records intent but no *predicted*
outcome; `WorkerFinished` carries a summary but no error signal.

**No homeostatic setpoint is read back as an error signal.** GOAL.md metrics are
genuine setpoints — "≥ 20 consecutive unattended iterations," "3/3 builds
demoable," "step-authoring rate → 0." But nothing in the loop *reads them back*.
They're aspirations in a header, not references against which the mind computes and
corrects error. A homeostat (Ashby) or a VSM (Beer) is defined by closing that
loop. The one place the design does close it is **cost-as-control-signal** (§3.6:
waves "pace, downshift models, or escalate against a `spend_cap`") — which proves
the pattern is possible and shows how conspicuously it's absent everywhere else.

**Memory curation is note-taking, not surprise-minimizing compression.** MEMORY.md
is curated by "what's useful," but nothing weights an entry by *how much it would
change a future decision* — the FEP notion that you keep what most reduces future
prediction error. Without that, "curation discipline" is a good intention with no
gradient; the Dumb Zone is held off by taste, not by a principle.

**No exploration/exploitation term in dispatch (the dark-room risk).** Dispatch is
pure pragmatic value: advance the roadmap. There is no *epistemic* value — no
dispatch whose purpose is to *reduce uncertainty* about an approach (spike it,
learn, discard). Fork-to-explore (§3.2) is the natural home for this but it's
vision, not live. The failure mode FEP predicts: a wave with a vague GOAL grinds
low-surprise busywork — the dark room — because nothing rewards information gain,
only motion. "Unattended loop iterations" as a metric can even *reward* the dark
room.

**The light cone is fixed, not expanding.** Levin/"care": intelligence grows by
enlarging the sphere of concern. A wave's concern is pinned by its GOAL/roadmap;
children partition work but don't *broaden* the parent's concern. This is more
philosophy than defect — flagged so it isn't mistaken for a code gap.

## 4. Concrete, adoptable implications

Rated **adopt-now** (cashes out in small code), **vision** (real but a project),
**merely-poetic** (does not cash out — flagged so we don't build it).

1. **Predicted outcomes + surprise on worker completion. (adopt-now, mechanism.)**
   Add an `expectation: String` to `WorkerDispatched` (the mind states what it
   expects before dispatch) and, on `WorkerFinished`, a cheap surprise check
   (expected vs. summary). Large surprise → auto-route to MEMORY curation + an
   AttentionItem. Turns the mind from reactor into minimal predictive controller.
   Small change to two journal events; the *judgment* of surprise can start as the
   mind's own read. (Honest caveat: using the signal *well* is vision; wiring the
   signal is adopt-now.)

2. **Metrics as live error signals in the heartbeat seed. (adopt-now.)** The
   heartbeat already fires when quiet. Have it read GOAL.md's metrics as setpoints
   and fold the *gap* into the seed: "8/20 unattended iterations, trending down;
   step-authoring rate 2, target 0." This is Beer's homeostat / VSM System 3
   closing the loop, and it's nearly free — the metrics and the tick both exist.
   Highest value-to-cost item here.

3. **Precision-weight the algedonic channel. (adopt-now, small.)** Escalation is
   effectively binary today. Add a confidence/precision field so only
   high-precision surprise propagates child→parent→root→human — the FEP refinement
   of Beer's alarm bus, and the antidote to escalation spam as wave trees grow.

4. **Epistemic dispatch (exploration budget). (vision.)** A placement/flag where a
   worker is dispatched to *reduce uncertainty* (spike an approach) vs. to make
   progress — epistemic vs. pragmatic value. Natural extension of the `fresh |
   pool | stack` placement axis and the fork-to-explore vision (§3.2). Real, but a
   design in its own right.

5. **Surprise-weighted memory pruning. (vision.)** Weight MEMORY.md entries by
   estimated decision-impact; prune low-information notes on a cadence. Hard to make
   rigorous without a model of "future decisions," so: vision, not now.

6. **Formalizing the "does this create a center?" test as Markov-blanket
   integrity. (merely-poetic.)** The vocabulary aligns beautifully and it's worth
   *saying* — but the test already works as prose. Building a variational-free-
   energy calculator over wave boundaries, or literally computing a blanket, buys
   nothing. Keep the intuition; don't math it. Likewise "MEMORY.md is a generative
   model" (it's a description, not a predictor) and "expand the wave's light cone as
   care" (no code move) are poetic — named here so they don't sneak into a roadmap.

**The through-line:** the wave design already has the *structure* this tradition
describes — sovereign boundaries, nested levels with two-way causation, an
algedonic bus, a compressed model of "what we know." What it lacks is the
*dynamics*: prediction, error, and setpoint-correction. It's a beautifully
partitioned system that doesn't yet close its control loops. Items 1–3 close them
cheaply; the rest is vision or should stay a metaphor.

## 5. Sources

- Friston, *The free-energy principle: a unified brain theory?* — https://www.nature.com/articles/nrn2787
- SSC, *God Help Us, Let's Try To Understand Friston On Free Energy* — https://slatestarcodex.com/2018/03/04/god-help-us-lets-try-to-understand-friston-on-free-energy/
- Friston et al., *The Markov blankets of life* — https://royalsocietypublishing.org/rsif/article/15/138/20170792
- *Making the Thermodynamic Cost of Active Inference Explicit* — https://www.mdpi.com/1099-4300/26/8/622
- *Towards a Geometry and Analysis for Bayesian Mechanics* — https://arxiv.org/abs/2204.11900
- Levin, *Technological Approach to Mind Everywhere (TAME)* — https://www.frontiersin.org/journals/systems-neuroscience/articles/10.3389/fnsys.2022.768201/full
- Doctor, Witkowski, Solomonova, Duane, Levin, *Biology, Buddhism, and AI: Care as the Driver of Intelligence* — https://www.mdpi.com/1099-4300/24/5/710
- Noble, *A theory of biological relativity: no privileged level of causation* — https://royalsocietypublishing.org/doi/10.1098/rsfs.2011.0067
- *Causal Emergence 2.0* — https://arxiv.org/abs/2503.13395
- Softmax inspiration index — https://softmax.com/blog/inspiration
- Stafford Beer, Viable System Model / algedonic channel (implicit ancestor; see `lfd/types/attention.rs`)
