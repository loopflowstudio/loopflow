# Redesign the design step: dream big, then size-check

## What to build

A new design step that lets you explore the full idea before deciding scope, then forks into either "ship as one commit" or "roadmap it" based on size.

## What changes

One file: `rust/loopflow/src/engine/builtins/steps/interactive/design.md`

The existing roadmap, add-to-roadmap, kickoff, and ingest prompts stay untouched.

## The new design flow

The conversation has four phases. Phases 1-2 replace the current "ask what, constrain scope" opening. Phases 3-4 replace the current "write spec, commit, end."

### Phase 1: Dream

Same conversational opening — "What are you trying to build?" But no scope pressure. No "what's the smallest version?" Don't constrain yet. Let the user describe the full vision.

> "dream big"

### Phase 2: Detail

Walk through components in detail. Data structures, functions, interactions, edge cases. This may take a while — keep writing to `scratch/<branch>.md` as you go so nothing gets lost.

Same "write as you go" principle as current design. Same emphasis on code sketches and quoting the user verbatim. The difference is you're detailing the *whole* idea, not a pre-scoped slice.

> "go through different components in detail. It may take a while. keep jotting things down in case you get lost."

### Phase 3: Size-check

After the idea is fully detailed, evaluate two signals:

1. **Design doc size** — is the spec exceeding ~1000 words? If the design itself is big, the implementation will be bigger.
2. **Implementation size** — would this be ~1000+ LOC? That's generous for a single commit.

Either signal suggests roadmapping. Bias toward "yes it fits" when it's close — single commits are preferable. But these are heuristics, not rules. The user can override.

> "~1000 words max is another constraint similar to 1000 LOC — if it doesn't fit, indicator maybe better to roadmapify (not always, user can override)"
> "bias towards single commits if it does fit, and then once we're breaking it up be more aggressive about committing frequently"

### Phase 4: Fork (explicit checkpoint)

Present the size assessment to the user and ask explicitly: **"This looks like it fits in one commit — proceed to implement?"** or **"This is bigger than one commit — want me to break it into a roadmap?"**

This is the natural session exit point. The user's answer determines what command to run next.

> "Choice to roadmap or just proceed to implement should be explicit and a good way of the agent signaling to the user it's a good time to exit the session and run the next command"

**If the user says implement:**
- Tighten the scratch doc into the standard design spec (What to build, Data structures, Key functions, Constraints, Done when)
- Commit and end. User runs `lf implement` next.

**If the user says roadmap:**
- Break the idea into staged roadmap items
- Write `scratch/roadmap-proposal.md` following the existing roadmap output format (status, context, scope, approach)
- The first stage becomes the design doc for this branch (`scratch/<branch>.md`)
- Remaining stages are proposals for `roadmap/`
- Commit and end. User runs `lf add-to-roadmap` next to promote remaining stages.

## Prompt structure

The new prompt keeps the same frontmatter, "Who reads this", and "What makes a good design doc" sections. Changes:

- **Workflow** — rewritten to the 4-phase structure above
- **Required sections** — kept, but applied after phase 3 (not from the start)
- **Conversation guidance** — remove "what's the smallest version that's useful?", remove "bias toward brevity", add "dream big, detail fully, then size-check"

## Constraints

- The design prompt is the only file that changes. Roadmap/add-to-roadmap prompts stay as-is.
- The "Who reads this" framing stays — design docs are for humans and LLMs.
- "Write as you go" stays — crash recovery matters.
- The roadmap fork should feel like a natural continuation of the conversation, not a mode switch.

## Done when

`cargo test -p loopflow golden_prompt` passes with the updated design prompt, and `cat rust/loopflow/src/engine/builtins/steps/interactive/design.md` shows the four-phase structure.
