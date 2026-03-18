# Reframe reviews: demo-first, intent-first

Replace the evaluation-oriented `review` and `review-design` steps with steps that serve the human's understanding and intent.

## What to build

Three new interactive steps, one branch construct, and flow updates.

### `demo` step

Experience-first walkthrough of observable changes. The agent runs things, shows output, walks the human through what changed *as a user would experience it*. Code comes after, and only if something felt off or the human wants to dig in.

```
Flow: read diff → identify what's demoable → run/show it → react together → optionally look at code
```

**"Demoable" means:** a command to run, output to see, UI to click, behavior to observe. If you can't point at something and say "look, this is different now," it's not a demo — it's a code-review.

### `code-review` step

Code-design-focused walkthrough of structural changes. For refactors, internal quality improvements, architectural reshaping. No pretense of "demoing" — walk through the decisions and why this shape is better.

Goes beyond the diff. The human uses this session to focus on how the change integrates with surrounding code and whether it's the right next step in the larger architectural vision. The diff is the starting point, but the conversation is about trajectory — "does this pull the codebase toward where it wants to go? What would the next step look like?"

### Branch: `demo | code-review`

The agent reads the diff and routes. Most feature work → `demo`. Pure refactors → `code-review`. Mixed PRs get the agent's best judgment (probably `demo` with a code section after).

```yaml
# replaces `review` in flows
- branch:
    paths:
      demo:
        step: demo
        description: "Change produces observable behavior — walk through the experience"
      code-review:
        step: code-review
        description: "Internal/quality change — walk through the code design"
```

### `review-design` rewrite

Same step name, different energy. The kickoff step elaborated a fuzzy roadmap item — that elaboration is AI-generated. This session is the human reshaping it.

The agent comes loaded (wave context, roadmap item, kickoff output in scratch/) and presents its understanding: "Here's what I think you meant." The human sculpts it. The session serves the user expressing what they want, not evaluating a pre-existing design.

Key shift: from "pressure-test the design" to "reshape the AI's elaboration into your actual intent."

## Constraints

- `gate` stays unchanged — headless ship check, different job
- `refine` stays unchanged — text refinement, different job
- `tend/review-chord` stays unchanged — different domain
- Branch syntax follows existing `qa-deploy.yaml` pattern

## Flow updates

**`ship.yaml`**: `review` → branch (demo | code-review)

**`ship-roadmap-build.yaml`**: `review` → branch (demo | code-review), `review-design` keeps its name but gets the rewritten prompt

## Done when

- `demo.md` exists in `steps/interactive/`
- `code-review.md` exists in `steps/interactive/`
- `review-design.md` is rewritten with intent-capture framing
- `review.md` is deleted (replaced by the branch)
- `ship.yaml` and `ship-roadmap-build.yaml` use the branch construct
- Prompts follow PROMPT_STYLE.md
