Explore ambitious changes to the project's scope or direction.

This is for bigger swings—new capabilities, architectural shifts, or directions that could meaningfully change what the project does or how it works.

## What to consider

**New capabilities.** What could this project do that it doesn't? What adjacent problems could it solve? What would make users say "I didn't know I needed this"?

**Architectural evolution.** What constraints are baked into the current design? What would a v2 look like if you could start fresh? What's the path from here to there?

**Integration opportunities.** What other tools or systems could this connect with? What workflows would that enable?

**Scale and scope.** What happens if usage grows 10x? What if the use case expands beyond the original intent?

## How to work

Start by understanding the current design thoroughly. Read the key files. Understand the constraints.

Then propose one significant change. Not a list of ideas—a specific direction with:

- What it enables that isn't possible today
- What it would take to build
- What risks or downsides it introduces
- A rough path to get there

If the change is compelling, start building it. Create a design doc under `.design/` if the scope is large enough to warrant one.

## What makes a good expansion

**Multiplicative value.** Changes that make everything else better, not just add one more thing.

**Natural fit.** Extensions that feel like they were always meant to be there, not bolted on.

**Tractable scope.** Ambitious but achievable. Something you could make real progress on in one session.

## What to avoid

Don't propose changes just because they're technically interesting. The question is whether they make the project more useful.

Don't chase trends. The project should be better at being itself, not more like something else.

Don't ignore constraints. Understand why things are the way they are before proposing to change them.

## Auto mode

In auto/headless runs, do not pause to ask questions. Make the best assumption you can and append any open questions to `.design/questions.md`.

