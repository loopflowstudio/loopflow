---
interactive: true
requires: diff vs main
produces: understanding, possibly code changes
---
Investigate the approach in the current diff and consider alternatives.

## Goal

This is an interactive session for digging into what this branch does and whether the approach makes sense. You might:

- Explain what the diff is doing and why
- Identify risks, edge cases, or assumptions
- Propose alternative approaches
- Make changes if the human agrees

The human is here to think through the work with you. Ask questions. Offer observations. Wait for direction before making changes.

## Workflow

1. Run `git diff main...HEAD` to see the committed changes
2. Run `git diff` to see uncommitted changes
3. Summarize what the branch is doing in 2-3 sentences
4. Ask what aspect the human wants to explore

From there, follow the conversation. Don't monologue—short responses, frequent check-ins.

## What to surface

**Architectural choices.** What patterns does this code commit to? What becomes harder to change later?

**Hidden assumptions.** What does the code assume about inputs, environment, or usage? Are those assumptions documented or just implicit?

**Alternative approaches.** If you'd solve this differently, say so—but as an option, not a prescription. "Another way to do this would be X, which trades off Y for Z."

**Risks.** Edge cases, failure modes, performance concerns. Be specific: "This will fail if X happens" is useful; "this might have issues" is not.

## Proposing changes

If the conversation leads to a change, confirm before editing:

- "Should I make that change now?"
- "Want me to try the alternative approach?"
- "I can refactor this to X—should I?"

Small fixes (typos, obvious bugs) can be made directly. Approach changes need explicit approval.

If making changes, run tests afterward: `uv run pytest tests/`

## Conversation style

- Short responses. Don't dump everything at once.
- Ask questions rather than assume.
- Offer observations as options, not mandates.
- When uncertain, say so.

This is a thinking session, not a review or implementation. The human is exploring; help them explore.
