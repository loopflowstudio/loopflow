---
interactive: true
requires: scratch/<slug>.md (elaborated design from kickoff)
produces: scratch/<slug>.md (approved or revised) | direction to iterate
---
Walk the human through the design before implementation burns time.

Kickoff produced a bold, opinionated design. This step checks whether the boldness points in the right direction. The design doc is a bet — this is the last cheap moment to change it.

## Arc

Each phase is a conversation pause. Present findings, wait for reaction, adjust. Don't monologue through all phases — pause after each and let the human steer.

### 1. Orient

Summarize the design in 2-3 sentences. What problem it solves, the approach chosen, the key bet. Then state your honest first reaction — what excited you, what worried you.

### 2. Scope check

Is this the right size? One commit or should it break further? Check:

- Does the design try to do too much in one pass?
- Are there natural seams where it could split into independently shippable pieces?
- Is anything marked "in scope" that could wait?

If scope looks wrong, say so now. Scope changes after implementation started are expensive.

### 3. Model

Walk through the central data structures and APIs the design proposes. Are the names right? Are the boundaries between types right? Does the type hierarchy match how users think about this?

Read existing code that this design extends or replaces. Does the proposed model fit naturally, or does it fight the existing architecture?

### 4. Decisions

Surface the key decisions and their alternatives. Kickoff's "Alternatives considered" table is the starting point, but probe deeper:

- Are there approaches kickoff didn't consider?
- Do the tradeoff assessments hold up?
- Is the chosen approach bold enough, or did it default to safe?
- Is it too bold — betting on something unproven when a known solution exists?

Frame each as "here's the tradeoff" not "here's what's wrong."

### 5. Failure modes

Kickoff imagined wild success and wild failure. Stress-test those:

- What's the most likely way this fails in practice?
- What assumption, if wrong, would require a rewrite?
- What dependency could block progress mid-implementation?

Name the single biggest risk. Ask the human if they're comfortable with it.

### 6. Verdict

After the conversation, land on one of three outcomes:

**Ship it.** The design is solid. Move to implementation. Note any minor adjustments discussed — update the design doc inline before ending.

**Iterate.** The direction is right but the design needs rework. Summarize what needs to change and why. The human can either revise the doc themselves or re-run kickoff with the new constraints.

**Rethink.** The approach is wrong. Capture what we learned about why, so the next attempt doesn't repeat the same dead end. Write learnings into `scratch/questions.md`.

State the verdict clearly. If ship, update `scratch/<slug>.md` with any adjustments from the conversation.

## Guidance

- This is a design review, not a code review. Focus on intent, model, and scope — not implementation details the implementing session will figure out.
- The design doc is for the implementing agent. If something is ambiguous enough that you'd guess wrong, flag it now.
- Don't propose alternatives without sketching them. "Have you considered X?" is useless. "X would look like [sketch] and trade Y for Z" is useful.
- Respect the design's boldness. Kickoff is opinionated by design. Don't sand down every sharp edge — only flag decisions where the risk outweighs the upside.
- If the wave context exists, check the design against wave Goals and Risks. A design that ignores known risks should be called out.

## Wave alignment

If `<lf:wave>` is present:

- Does this design advance the wave's stated Goals?
- Does it respect scope boundaries from the wave README?
- Does it introduce risks the wave already flagged?
- Will the "done when" criteria actually move the wave forward?
