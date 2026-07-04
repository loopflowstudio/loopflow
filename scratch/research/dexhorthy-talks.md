# Dex Horthy beyond 12-Factor: talks & essays

Canonical text is HumanLayer's **`ace-fca.md`** (writeup behind "Advanced Context
Engineering for Agents").

## Primary sources
- **ace-fca.md** — *Advanced Context Engineering for Coding Agents*: https://github.com/humanlayer/advanced-context-engineering-for-coding-agents/blob/main/ace-fca.md
- **Talk** — *No Vibes Allowed: Solving Hard Problems in Complex Codebases*: https://www.youtube.com/watch?v=rmvDxxNubIg
- **YC talk** — *Advanced Context Engineering for Agents*: https://www.youtube.com/watch?v=IS_y40zY-hc
- **Humans in the Loop** interview: https://thehumansintheloop.substack.com/p/making-agents-mainstream-for-dev-with-dexter-horthy
- **Dev Interrupted / LinearB** — *Ralph, RPI, and the "Dumb Zone"*: https://linearb.io/dev-interrupted/podcast/dex-horthy-humanlayer-rpi-methodology-ralph-loop
- **QRSPI** writeup: https://alexlavaee.me/blog/from-rpi-to-qrspi/

## 1. Context engineering & compaction
Frame: **an LLM is a stateless function; context quality is the only input you
control.** "No matter how big context windows get, you always get better results
if you use less of them."

- **The 40% rule / "dumb zone."** Quality degrades as the window fills; empirically
  starts failing ~**40% utilization**; target band **40–60%** by complexity.
  Beyond that the model quietly stops reasoning and you aren't told.
- **Intentional compaction** (the technique F3 only gestures at):
  1. When a session gets long/drifts, don't keep appending — have the agent
     **write state into a markdown artifact** (end goal, approach, steps done,
     blockers).
  2. **A human validates that summary.**
  3. **Start a fresh context window seeded only with the validated artifact.**
  - **Keep**: relevant files *with line ranges* (`foo/bar.ts:120–340`), verified
    architectural facts, explicit constraints.
  - **Drop**: raw logs, tool traces, full file contents, search/glob output, big
    JSON tool responses, error-explanation prose.
  - Compaction "converts exploration into a one-time cost." Mid-task: compact
    status back into the plan file after each verified phase.
- **Leverage hierarchy:** bad code = a bad line; bad plan = hundreds of bad lines;
  bad research = thousands. Human attention goes to the *earliest* artifacts.
- **Instruction budget:** frontier models reliably follow only **~150–200
  instructions**; buried ones silently skipped. His fix: each step **<40
  instructions**.

## 2. Spec-first / "the spec is the hard part"
Thesis: **review the plan, not the code.** 2,000-line PRs daily are unsustainable;
a 200-line plan is feasible and is where architectural coherence is preserved.

Workflow **RPI → QRSPI** (Question, Research, Spec, Plan, Implement):
- **Research** deliberately *hides the ticket*, has the agent produce an objective
  codebase map — "read code, not docs" — before any opinion forms.
- **Design discussion** (~200 lines): "you get to do brain surgery on the agent
  before you proceed downstream."
- **Structure outline** enforces vertical slices (mock API → frontend → DB) +
  signatures before spec. Then Spec → Implement (mechanical once aligned).

Sharp caveat aimed at us: **"Plans that read well don't necessarily build well."**
Detailed specs *feel* like progress while masking wrong assumptions — "plans are
persuasive artifacts by nature." QRSPI forces verification (grounded in real code)
*before* the spec. **"Do not outsource the thinking. There is no magic prompt.
You the engineer are an important part of this process — seek leverage."**

## 3. The outer loop thesis
Inner loop = agent's own reason-act-observe; outer loop = managed, human-
checkpointed structure. Bet: **coding agents get commoditized; the durable value
is the workflow/outer loop.** Architecture he runs: a **parent agent shells to
sub-agents, resets context between phases** — faster/dumber models write code +
run tests, a bigger model spot-checks, keeping the parent's context low. The outer
loop's job is **staying in the smart zone** via ruthless context resets — the only
thing that matters, not elaborate orchestration.

**Ralph** (degenerate case): a dumb autonomous `while` loop running Sonnet
continuously. Hackathon: ~$10–11/hr/server, 6 servers, 6–8 hrs, cloned 6 sponsor
products overnight to ~90%. Verdict: **"If you're doing greenfield, just write the
specs and use Ralph."** RPI/QRSPI is for **brownfield**, where you can't tolerate
the agent diverging.

## 4. Reliability / evals / don't outsource the thinking
Not autonomous: "does not work if you're looking for the one magic prompt."
Reliability = **human checkpoints at highest-leverage boundaries** (research doc,
plan) + always able to **eject** ("something unexpected happened" is a first-class
outcome, not a crash). Contacting a human = another tool call in the trace (F7).
Proof points: 300k-LOC Rust bug fixed <1hr by a non-expert; 35k-LOC feature in 7
hrs; merges without rework; ~$12k/mo Opus for a team of 3. Notably he **doesn't
lead with formal evals** — the mechanism is *structural* (compaction + plan-review
+ eject).

## 5. Multi-agent / subagents
**Subagents are not roles — they are context forks.** Spawn one for a big
exploratory read that **returns a succinct factual summary**
(`"logic in foo/bar.ts:120–340, entrypoint BazHandler"`) so the parent's window
never fills with noise. When: file discovery, codebase summarization, dependency
analysis — token-heavy work whose *output* is small. **Constraint:** "sub-agents
as context forks only pay off if the parent trusts the summary blindly." If the
parent re-reads everything, you've gained nothing.

## What this implies for the wave server
Our design (outer loop + subagent tree + append-only compaction) is **unusually
well-aligned** — we've built the outer loop he says is the whole game. Conflicts
are on the two things he's most emphatic about: plan-before-implement and human-
at-the-plan-boundary.

### Adopt
1. **Make the event-log fold literally implement intentional compaction.** The
   fold that reconstructs a pass's context IS our compaction step — encode his
   keep/drop rules; make it *deterministic* where he does it by hand. Highest-
   leverage mapping.
2. **Budget the window; target 40–60%.** Measure per-pass fill; force compaction/
   reset near ~50% rather than running codex into the dumb zone. A knob the wave
   server owns.
3. **Enforce the instruction budget on GOAL.md + MEMORY.md + system prompt** —
   <150–200 combined, ideally <40 per pass. Two-tier MEMORY must be a *compaction*
   tier, not an accretion tier.
4. **Subagent passes return small, trustworthy summaries** (file:line anchors,
   verified behavior) the parent fold trusts without re-reading. Design the pass
   boundary as a context fork returning a summary, not a shared scratchpad.
5. **Split research/plan from implement.** Add a **research pass** (map the repo,
   read code not docs, ticket-blind) and a **plan artifact** checked *before*
   implementation passes dispatch. GOAL.md is the spec; ground it against the
   actual repo, don't treat it as sufficient.

### Conflicts to resolve
1. **Headless autonomy vs "do not outsource the thinking."** His reliability story
   is a human reviewing the plan; our auto/headless waves delete that human. His
   own resolution: autonomous looping is **Ralph mode** — greenfield, tolerate
   ~90%. Decide *per-wave*: Ralph-mode (lean in, don't pretend brownfield-reliable)
   vs brownfield-reliable (must checkpoint at the plan boundary — and with no live
   human, that becomes `scratch/questions.md` + an eject-to-human event, not a
   silent proceed). We have **no plan-review gate** today; that's the gap.
2. **"Plans read well but build well is different."** Don't treat a plan
   artifact's existence as green-light. Insert a design-discussion/structure-
   outline pass grounded in real code between spec and implement.
3. **First-class eject.** Define an *unexpected-state* transition (append an eject
   event, surface to human) rather than retrying into the dumb zone.

**Net:** he validates our architecture and hands us four tuning constants (40–60%
fill, <40 instructions/pass, file:line summaries, research-before-plan) plus one
unresolved fork: **are our waves Ralph (autonomous, 90%, greenfield) or brownfield-
reliable (needs a plan-review checkpoint)?** The headless design implicitly claims
the second while operating like the first.
