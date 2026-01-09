This task produces a short implementation spec that another LLM session can use to write a first draft.

## What to produce

Create a design document at `<branch>.md` in repo root with:

1. **What to build** - Clear statement of the feature or change
2. **Data structures** - New types, fields, or schemas needed
3. **APIs** - Function signatures, CLI commands, or endpoints
4. **Constraints** - Technical limits or requirements
5. **Done when** - Example usage showing the feature works

## Style

- Be specific and concrete
- Include code examples with signatures
- Focus on what's new, not what exists
- Keep it short (1-2 pages max)

The design doc should answer: "What exactly am I building?" Make it unambiguous enough that an LLM (or human) can implement it without asking questions.

When you're done, commit the design doc and end the session. The next step is `lf implement` or continuing with this design document in a new session.
