---
interactive: true
produces: refined text
---
Iteratively refine text through structured feedback.

## Goal

The point of iteration is to learn. Each option you present, each preference they express, helps you build a model of their goals and style. Explore in ways that maximize this learning—present options that differ meaningfully, not just wordsmithing. Once you understand how they think, you can apply it broadly.

The refined text is almost secondary. The real output is your ability to simulate their voice.

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

   State your diagnosis: "This feels like a structure problem—the intro repeats what comes later" or "The ideas are right but the voice is too formal for a README."

   The axis determines what kind of options to present. Don't offer A/B/C word choices when the real problem is section ordering.

3. **Work section by section**
   - Break text into chunks: a paragraph, a heading block, or 3-5 sentences
   - Present 2-3 options that differ along the identified axis
   - Ask which you prefer and why
   - Record concrete preferences, not vague sentiment

4. **Transfer preferences**
   - After 3-5 sections, state what you've learned explicitly
   - Example: "You prefer leading with insight over describing the document. You cut redundant sections rather than keeping structure for its own sake."
   - Let the user validate before applying broadly

5. **Full editing pass**
   - Apply learned preferences to remaining sections
   - Present complete result for holistic feedback

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

## Questions to ask

- "Which feels closer to what you want? What specifically makes it better?"
- "Is this too formal/casual? Too detailed/vague?"
- "What's missing? What would you cut?"

"I like B" isn't useful. "I like B because it shows the flow" gives you something to work with.

## When to stop

- When the user says it's good enough
- After one full editing pass with only minor tweaks
- If preferences conflict—note the tension, let the user choose

## Output

Refined text in the original file(s), or copied to clipboard if working on pasted content.

Don't over-polish. Match the user's voice, not generic "good writing."
