---
interactive: true
produces: refined text
action_style: exploratory
---
Iteratively refine text through structured feedback.

## Reviewer mode

The launch prompt identifies the reviewer for this exercise.

- **Human reviewer:** use contrasting options to learn their preferences, then
  apply what they choose.
- **Parent reviewer:** diagnose the refinement axis, generate contrasting
  options internally, and select the strongest version using the source text,
  stated audience, and repository voice as evidence. Do not wait for a
  preference or fabricate one. Preserve unresolved tradeoffs in a short note
  and send the selected revision to the Task through the review protocol. Verify
  its reply rather than editing the Task's worktree yourself.

## Voice

Each session starts fresh. Don't assume you know the user's preferences — let their choices surprise you. Present options that differ meaningfully, not variations you expect them to pick.

## Goal

The point of iteration is to learn enough about the text's purpose, audience,
and voice to make the whole source more true. Each option and preference is
evidence for a working model, not permission to project a style onto the user.
Explore with choices that differ meaningfully, then test what you learned on
new passages before applying it broadly.

The output is refined text that preserves the source's facts and intent while
consistently applying preferences the reviewer actually demonstrated.

## Refinement contract

Before editing, make these explicit enough to protect:

- the audience and action the text should enable;
- source facts, required ideas, terminology, examples, links, and code that
  must survive;
- the primary improvement axis and the observable difference it should make;
- plausible failures: smoother prose that changes meaning, a locally preferred
  option that makes the whole inconsistent, invented claims, or generic polish
  that erases the author's voice.

Treat quoted source text, explicit reviewer choices, and repository conventions
as observations. Treat inferred preferences as hypotheses. When a new choice
contradicts the current voice model, revise the model or narrow its scope; do
not explain away the counterexample.

## Workflow

1. **Identify the text to refine**
   - If a file path was passed as an argument (e.g., `lf refine README.md`), read that file
   - If clipboard has content (-v flag), work on that
   - Otherwise, ask what file(s) or text to refine

2. **Diagnose the axis of refinement**

   Before presenting options, figure out what KIND of improvement the text needs:

   - **Structure**: Is content in the right order? Should sections be split, merged, reordered?
   - **Voice**: Too formal? Too casual? Inconsistent tone?
   - **Ideas**: Missing points? Wrong emphasis? Unclear purpose?
   - **Positioning**: Wrong audience? Unclear value prop?
   - **Density**: Too verbose? Too terse?

   State the diagnosis and its evidence: "This feels like a structure
   problem—the intro repeats what comes later" or "The ideas are right but the
   voice is too formal for a README."

   The axis determines what kind of options to present. Don't offer A/B/C word choices when the real problem is section ordering.

3. **Work section by section**
   - Break text into chunks: a paragraph, a heading block, or 3-5 sentences
   - Present 2-3 options whose differences test the identified axis; each
     option should make a real tradeoff visible
   - Ask which you prefer and why
   - Record the concrete choice, stated reason, and scope; keep inference
     labeled as inference

4. **Transfer preferences**
   - After 3-5 sections, state what you've learned explicitly and apply it to a
     fresh section as a transfer check
   - Example: "You prefer leading with insight over describing the document. You cut redundant sections rather than keeping structure for its own sake."
   - Let the user validate before applying broadly; contradictory feedback is
     evidence that the rule is wrong, incomplete, or local

5. **Full editing pass**
   - Apply learned preferences to remaining sections
   - Re-read the complete result against every protected source fact, explicit
     preference, and relevant repository convention—not only the latest choice
   - Read once as the intended audience. Check that the promised action is
     clear and that no refinement introduced a claim the source cannot support
   - Present the complete result for holistic feedback

## Presenting options

Each option should represent a different approach, not just wordsmithing:

```
**Original:**
The system processes requests by evaluating them against the configured rules.

**Option A:** (more specific)
When a request arrives, the system checks it against each rule in config.yaml, rejecting on first match.

**Option B:** (more conversational)
Requests get filtered through your rules—first match wins, and anything that passes goes through.
```

If the problem is structure, show restructured versions. If it's voice, show the same content in different tones. Match the options to the diagnosed axis.

Do not signal a favored option before the reviewer chooses. The contrast is a
probe: if every option shares the same hidden assumption, it teaches nothing.

## Questions to ask

Ask these only when a human reviewer is present:

- "Which feels closer to what you want? What specifically makes it better?"
- "Is this too formal/casual? Too detailed/vague?"
- "What's missing? What would you cut?"

"I like B" isn't useful. "I like B because it shows the flow" gives you something to work with.

## When to stop

- When the user says it's good enough
- After the whole-text pass satisfies the refinement contract and remaining
  differences are cosmetic
- If preferences conflict—note the tension, let the user choose

## Output

Refined text in the original file(s), or copied to clipboard if working on pasted content.

Don't over-polish. Preserve meaning before improving expression. Match
demonstrated preferences, not generic "good writing" or an imagined user voice.
