---
interactive: true
---
Iteratively refine text through structured feedback.

This is an interactive session for improving prose—prompts, design docs, README sections, or any text worth polishing. The goal is to learn your preferences through small choices, then apply them broadly.

## Workflow

1. **Identify the text to refine**
   - If clipboard has content (-v flag), work on that
   - Otherwise, ask what file(s) or text to refine

2. **Work section by section**
   - Break text into small chunks: a paragraph, a `# Heading` block, or 3-5 sentences
   - For each section, present 2-3 rewrite options
   - Ask which you prefer and why
   - Record what resonates: tone, structure, word choice, specificity level

3. **Transfer preferences**
   - After 3-5 sections, synthesize what you've learned
   - State patterns explicitly: "You prefer active voice, concrete examples, and shorter sentences"
   - Apply these patterns to remaining sections

4. **Full editing pass**
   - Rewrite the entire text incorporating learned preferences
   - Present the complete result for holistic feedback
   - If major issues remain, restart the section-by-section process

## How to present options

For each section, show the original and 2-3 alternatives. Keep them meaningfully different:

```
**Original:**
The system processes requests by evaluating them against the configured rules.

**Option A:** (more specific)
When a request arrives, the system checks it against each rule in config.yaml, rejecting on first match.

**Option B:** (more conversational)
Requests get filtered through your rules—first match wins, and anything that passes goes through.

**Option C:** (more technical)
Request processing follows a chain-of-responsibility pattern: rules evaluate sequentially until one claims the request.
```

Don't offer variations that are just wordsmithing. Each option should represent a different approach: level of detail, formality, structure, or emphasis.

## Questions to ask

- "Which feels closer to what you want? What specifically makes it better?"
- "Is this too formal/casual? Too detailed/vague?"
- "What's missing? What would you cut?"
- "Does this match how you'd explain it verbally?"

Record concrete preferences, not vague sentiment. "I like B" isn't useful. "I like B because it shows the flow, but 'first match wins' is unclear" gives you something to work with.

## Transferring preferences

After a few sections, explicitly state what you've learned:

> "Based on your feedback, you seem to prefer:
> - Active voice over passive
> - Concrete examples over abstract descriptions
> - Technical precision but conversational tone
> - Shorter paragraphs with clear structure
>
> I'll apply this to the remaining sections. Correct me if I'm wrong."

This lets the user validate your understanding before you apply it everywhere.

## When to stop

- When the user says it's good enough
- After one full editing pass with only minor tweaks requested
- If preferences conflict irreconcilably—note the tension and let the user choose

## Output

The deliverable is refined text in the original file(s), or copied to clipboard if working on pasted content.

Don't over-polish. The goal is to match the user's voice, not produce generic "good writing."
