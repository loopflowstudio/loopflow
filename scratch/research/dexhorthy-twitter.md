# Dex Horthy (@dexhorthy) — short-form + the event-driven thesis

**Access note:** direct `x.com` fetches returned HTTP 402; nitter/threadreader
mirrors unreachable. Tweet *text* below recovered via search indexing; long-form
canon (`ace-fca.md`, `12-factor-agents`, BoundaryML "ai that works" episodes) was
fetchable and holds the sharpest quotes. His tweets are mostly pointers back to
these.

## What he's saying (by topic)
1. **Outer loop / supervision.** Banner: *"The future of AI Applications is not
   gonna be humans sitting at a chat interface, the future is 'outer loop' or
   'headless' agents."* Built **ACP (Agent Control Plane)**, "a distributed agent
   scheduler… for outer-loop agents that run without supervision." The "The Outer
   Loop" Substack is named for this. Bet: agents that run headless and *escalate*
   to humans, not agents you babysit in a chat box.
2. **Context engineering / compaction** (his center of gravity). From `ace-fca.md`:
   *"the contents of your context window are the ONLY lever you have to affect the
   quality of your output."* Priority order for what goes in context: **Correctness
   > Completeness > Size > Trajectory.** Rule: design the *entire workflow* around
   context management; **keep utilization 40–60%.** Above → the **"Dumb Zone"**
   (his coinage) where recall degrades. Practice: **Frequent Intentional
   Compaction** — compact deliberately between phases into structured artifacts.
   Proof: 35k lines into a large Rust codebase in one 7-hr session. Subagents:
   *"sub-agents aren't about role-playing; they're about context control."*
3. **Spec-first / review plans not code.** RPI (Research → Plan → Implement), each
   stage compacting into an artifact with fresh context. *"A bad line of a plan
   could lead to hundreds of bad lines of code. And a bad line of research… could
   land you with thousands of bad lines of code."* Code review's real purpose (via
   Blake Smith) = *"mental alignment — keeping the team on the same page,"* which
   breaks at 20k-line PRs. **But he rejects spec-maximalism** — tweet (status
   2033392483674264044): *"the title 'spec is the new code' harks back to the 'all
   the code is assembly' idea which is WAYY far out. If you start behaving like
   you'll be able to ship and maintain production software without reading the
   code, you're in for a hard [time]."* Specs are durable artifacts; they don't
   excuse not reading code.
4. **CodeLayer / product.** "Post-IDE IDE," *"Superhuman for Claude Code"* — for
   people ready to move past vibe coding. Tweet (1993380051866517915): *"Live
   coding with CodeLayer, we'll use Research Plan Implement live to ship 3 new
   features."* The product *is* RPI + intentional compaction as a harness. Why
   agents fail: not weak models — *"the instructions are ambiguous and the agent
   harness is too weak."* The harness is the product.
5. **Architecture hot takes — the 12 factors + event-log thesis.** Most load-
   bearing find: his Nov 2025 "ai that works" episode on **event-driven agentic
   loops**: *"treat the backend like a game server. Every interaction is an
   append-only event, and each consumer — LLM loop, UI, persistence — receives a
   projection that suits its contract."* Rationale: *"Linear agent loops crumble
   once you need interrupts, approvals, or queued inputs; events give you a single
   truth you can replay."* Multi-consumer subtlety: *"the UI should show pending
   approvals, while the LLM should never see queued user messages until they are
   active."* Domain logic stays a **pure reducer**; messages queue during
   streaming and flush when complete; the same log replays deterministically for
   tests.

## Through-lines → implications for the wave server
1. **The harness owns the loop, not the model.** → Keep the deterministic outer
   loop in loopflow, not delegated to codex. Strongest structural endorsement of
   the wave-server shape.
2. **Context is the only lever; run passes at 40–60% and compact between them.** →
   Each bounded pass starts near-fresh, sub-Dumb-Zone; the outer loop's job between
   passes is Intentional Compaction (distill, don't carry raw history).
3. **Append-only event log, projections per consumer, pure reducer.** → Validates
   the event-log direction outright, and adds specifics to steal: one log, three
   projections (agent-loop context, human chat/UI, persistence); the agent should
   **NOT see queued human messages until a pass boundary**; steering enqueues
   events rather than mutating live state. Replayability → deterministic tests of
   the outer loop without live model calls. **Bank this.**
4. **Human leverage is at plan/research, not the diff.** → The chat/steering
   surface presents plans + research artifacts for approval at pass boundaries, not
   diffs for line review. Cheapest place to catch a wave going wrong is before
   implementation.
5. **Escalate to humans as tool calls, not a mode switch.** → Model human steering
   as an event/tool the wave emits ("contact human") that suspends the pass;
   Launch/Pause/Resume = simple APIs over the log (pause = stop projecting new
   agent events, resume = replay from cursor).
6. **Unify execution + business state** in one log — "where is the wave" and "what
   did it do" from the same source.
7. **Spec-first, but the code still gets read** — durable spec/plan artifacts, but
   surface the diff as a reviewable artifact even while steering happens upstream.
8. **Small focused passes; subagents for context control, not personas.**

## Sources
- ace-fca.md: https://github.com/humanlayer/advanced-context-engineering-for-coding-agents/blob/main/ace-fca.md
- 12-factor-agents: https://github.com/humanlayer/12-factor-agents
- "ai that works" — Event-driven agentic loops (Nov 5 2025, BoundaryML): https://boundaryml.com/podcast/2025-11-05-event-driven-agents
- Tweet "spec is the new code" critique — status 2033392483674264044 (text via search index)
- Tweet live-coding CodeLayer w/ RPI — status 1993380051866517915
- The Outer Loop (Substack): https://theouterloop.substack.com/
- Ralph / RPI / Dumb Zone (LinearB): https://linearb.io/blog/dex-horthy-humanlayer-rpi-methodology-ralph-loop
