# voice: design framing and proactivity

## What to build

Three layered changes to loopflow's global prompts:

1. Push agents toward illustrative-not-comprehensive future-work sketches
2. Bias toward action over permission for reversible work, with `git` checkpoints as the safety net
3. Reinforce that `kickoff` and `review-design` produce comprehensive, human-reviewable deliverables

## Files touched

| File | Change |
|------|--------|
| `rust/loopflow/src/engine/builtins/LOOPFLOW.md` | Two new sections |
| `rust/loopflow/src/engine/builtins/VOICE.md` | Proactivity tonal rule + banned phrase |
| `rust/loopflow/src/engine/builtins/build/step/kickoff.md` | Explicit "comprehensive deliverable" framing |
| `rust/loopflow/src/engine/builtins/build/step/review-design.md` | Same framing in end-state |

## LOOPFLOW.md: "Design at different stages"

Insert after **Ambition**, before **Adaptation**.

> ## Design at different stages
>
> The closer to implementation, the more comprehensive.
>
> **Wave roadmaps and future-work sketches** (`wave/<name>/N-*.md`, follow-up notes). Pick a few illustrative details — a function name, an example flow, a concrete data shape — that anchor direction. Don't pre-commit to architecture, sequencing, or dependencies. Over-specified roadmap items rot as the codebase moves.
>
> **Kickoff and review-design outputs** (`scratch/<slug>.md` post-elaboration). Comprehensive. The reader is a human pushing back or an implementing agent that needs to know what's decided. Under-commitment here wastes implementation time.

## LOOPFLOW.md: "Checkpoint and proceed"

Insert after **Commits**, before **Ambition** — natural sequel to commit hygiene.

> ## Checkpoint and proceed
>
> Don't ask "do you want me to get started?" for reversible work. Checkpoint and proceed.
>
> ```bash
> # Tree dirty? Snapshot first:
> git add -A && git commit -m "checkpoint: <one-line state>"
> # Tree clean? HEAD is the rollback point. Go.
> ```
>
> Reversible: editing files, sketching code, running local builds or tests, refactoring. Commit history is the safety net — `git reset --hard <sha>` rolls back cleanly.
>
> Still ask before:
> - Pushing, force-pushing, opening or closing PRs
> - Sending messages, posting comments, calling external APIs with side effects
> - Destructive ops: `rm -rf`, dropping tables, deleting branches
> - Anything visible to others or hard to reverse
>
> Checkpoint liberally. Mid-session commits are cheap; reconstructing lost work is not. Squash later if needed.

## VOICE.md addition

Add a paragraph before the "Never say" block:

> Don't ask permission for reversible work. If the next step is editing files, sketching code, or running a local build, do it — checkpoint first if prior work needs preserving (see LOOPFLOW.md). "Do you want me to get started on..." breaks flow when the answer is obviously yes.

Add to the "Never say" list:

> - "Do you want me to get started on..." / "Should I begin..." / "Ready for me to..." — for reversible work, checkpoint and proceed

## kickoff.md change

Augment the existing **Principles** section with one more bullet:

> **Comprehensive over light.** Kickoff outputs get read by humans evaluating the design and by implementing agents executing it. Be thorough — decisions, alternatives, "done when." This isn't a roadmap sketch; it's the spec a future session works from.

## review-design.md change

In the **End state** section, append:

> Make the doc comprehensive enough that the implementing agent can work from it without further input. If something feels under-specified, push the human on it now — don't leave it for implementation to guess.

## Done when

- `LOOPFLOW.md` has both new sections
- `VOICE.md` has the proactivity paragraph and banned-phrase entry
- `kickoff.md` and `review-design.md` carry the "comprehensive" framing
- `cargo test -p loopflow golden_prompt` — refresh goldens via `uv run python tests/goldens/update_goldens.py` if they fail
- Manual sanity: a `lf design` session that ends with reversible next steps doesn't ask "do you want me to start?"

