---
produces: refined text
action_style: exploratory
---
Refine text by offering contrasting options and learning from the choices.

## Reviewer mode

The launch prompt names the reviewer.

- **Interactive reviewer:** offer contrasting options to learn what they want,
  then apply what they choose.
- **Parent reviewer:** diagnose the axis, generate contrasting options
  internally, and pick the strongest using the source text, its audience and the
  repository's voice. Don't wait for a preference or make one up. Note any open
  tradeoff briefly and return the revision through the supplied review protocol.
  When reviewing for a Task, check its reply rather than editing its checkout.
  Without a review protocol, return the refined text in this conversation.

## Goal

Learn enough about the text's purpose, audience and voice to make the whole
source more true. Each session starts fresh: don't assume preferences, and
don't project a style. The result keeps the source's facts and intent and
applies the preferences the reviewer actually showed, consistently.

Before editing, pin down:

- the audience and what the text should let them do;
- facts, ideas, terminology, examples, links and code that must survive;
- the main axis of improvement and the difference a reader should notice;
- the likely failures: smoother prose that changes meaning, a local choice that
  makes the whole inconsistent, new claims the source can't back, generic
  polish that erases the author's voice.

Quoted text, the reviewer's choices and repository conventions are facts.
Inferred preferences are guesses. When a new choice contradicts your model of
the voice, revise the model or narrow it; don't explain the choice away.

## Workflow

1. **Find the text.** A file path argument (`lf refine README.md`), clipboard
   content (`-v`), or ask which file or text.

2. **Diagnose the axis.** Decide what kind of improvement the text needs:
   - **Structure**: order, splits, merges
   - **Voice**: too formal, too casual, inconsistent
   - **Ideas**: missing points, wrong emphasis, unclear purpose
   - **Positioning**: wrong audience, unclear value
   - **Density**: too verbose or too terse

   State it with the reason: "This is a structure problem: the intro repeats
   what comes later." Don't offer word choices when the real problem is section
   order.

3. **Work section by section.** Take a paragraph, a heading block or 3-5
   sentences. Offer 2-3 options that each make a real tradeoff on the axis. Ask
   which and why. Record the choice, the reason and its scope; label inferences
   as inferences.

4. **Transfer preferences.** After 3-5 sections, state what you've learned
   ("You lead with the insight, and cut sections that exist only for
   structure") and apply it to a fresh section as a check. Confirm before
   applying broadly; contradictory feedback means the rule is wrong, incomplete
   or local.

5. **Full pass.** Apply the preferences to the rest. Re-read the whole result
   against every fact that must survive, every stated preference and the
   repository's conventions, not just the latest choice. Read once as the
   audience: the action is clear and nothing claims more than the source.
   Present the complete result for feedback.

## Presenting options

Each option is a different approach, not wordsmithing:

```
**Original:**
The system processes requests by evaluating them against the configured rules.

**Option A:** (more specific)
When a request arrives, the system checks it against each rule in config.yaml, rejecting on first match.

**Option B:** (more conversational)
Requests get filtered through your rules—first match wins, and anything that passes goes through.
```

For structure, show restructured versions; for voice, the same content in
different tones. Don't signal a favorite before the choice. If every option
shares the same hidden assumption, the contrast teaches nothing.

## Questions to ask

Only with an interactive reviewer:

- "Which feels closer to what you want? What specifically makes it better?"
- "Is this too formal/casual? Too detailed/vague?"
- "What's missing? What would you cut?"

"I like B" isn't useful. "I like B because it shows the flow" is.

## When to stop

- When the reviewer says it's good enough
- When the full pass holds up and remaining differences are cosmetic
- When preferences conflict: name the tension and ask which way to go

## Output

Refined text in the original file, or on the clipboard for pasted content.
Don't over-polish. Meaning first, then expression. Match the preferences shown,
not generic "good writing".
