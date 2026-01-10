# Design: Preserving Design Intent Across LLM Sessions

> "I want to iterate on how we pass and update design intent across llm sessions."
> — user

> "Can you review our current implementation and ask me questions about how I might want to evolve things based on these ideas?"
> — user

## What to build

A minimal set of changes to how loopflow stores, loads, and frames design intent so that implementation and review stages treat specs as anchored guidance, not immutable source of truth.

## Current implementation review (as-is)

```text
design/                     # design docs auto-included in prompt context
  *.md                      # loaded ahead of README/STYLE

.claude/commands/design.md  # instructs to create design/ doc early
.claude/commands/review.md  # says: deliverable is written review in design/
.claude/commands/polish.md  # says: treat implementation as source of truth

src/loopflow/context.py
  - gather_prompt_components()
  - always loads design/ docs into <lf:docs> when present
  - design/ docs are inserted near top of docs for every task
```

Observations:
- Design docs are always present in context once created.
- Review/polish already say to treat implementation as source of truth, but the design docs still load for every task.
- There is no explicit distinction between intent vs design vs decisions.

## Data structures

```python
# One possible structure if we split intent from design.
class DesignIntent:
    path: Path          # e.g., design/intent.md
    frozen: bool        # True for "constitution"/intent docs

class DesignDoc:
    path: Path          # e.g., design/design.md
    evolving: bool      # True for live spec

class DecisionLog:
    path: Path          # e.g., design/decisions.md
    append_only: bool
```

## APIs

```python
def gather_design_docs(repo_root: Path) -> list[tuple[Path, str]]:
    """Return .design/*.md docs to include in prompt context."""

def clear_design_artifacts(repo_root: Path) -> None:
    """Remove .design contents, keep folder."""
```

CLI/task-level examples:
```text
lf design     # writes .design/*.md (design intent)
lf implement  # reads .design/*, updates code + .design/decisions.md
lf review     # reads code + .design/*, writes .design/review.md
lf polish     # rewrites primary design doc to match implementation
lf pr land    # clears .design/* contents before landing
```

## Constraints

- Must preserve existing loopflow task flow (design → implement → review → polish).
- Design docs should be committed on feature branches but removed before landing.
- Review must produce a review doc without deleting the original design doc.
- .design is preferred over docs/ for ephemeral design artifacts.
- Move design artifacts to `.design/` (no gitignore; still committed on feature branches).
- Multiple files are acceptable; prefer small, focused docs.
- Auto-include `.design/` in prompt context for all tasks.
- Landing removes `.design/*` contents but leaves the directory.

## Options to explore (from research doc)

```text
Option A: "Constitution" split
  - design/intent.md (immutable during impl)
  - design/design.md (evolves with code)
  - design/decisions.md (append-only)
  - review tasks only see intent + decisions, not design

Option B: Phase-specific context
  - do not include design/ during review/polish tasks
  - add explicit review rule in task prompt

Option C: Explicit evolution markers
  - add "Do Not Revert" section in design/ docs
  - review prompt instructed to treat this as authoritative

Option D: Gitignored working memory
  - design/ moved to .design/ and gitignored
  - only intent or summary is committed
```

## Market alignment (summary)

```text
Your approach: "spec-anchored with explicit evolution" + commit-on-branch, delete-on-land.

Matches:
- Spec-anchored workflows (Fowler): spec guides, code can diverge with intent tracked.
- Session separation: fresh reviewer vs implementer (common in practitioner patterns).
- Persistent handoff files: DECISIONS/PROGRESS/TODO variants align with working-memory patterns.
- Review-as-quality: reviewers focus on code quality, not spec compliance.

Differs:
- You do NOT want gitignored design; many workflows default to local-only scratchpads.
- You prefer committed design artifacts during dev, then removal before landing.
- You want review to produce a doc in the same design space (review.md), not a separate review system.

Implication:
- Loopflow should treat design docs as first-class, branch-scoped artifacts, not purely local scratchpads.
- Emphasize "intent tracking + evolution markers" rather than "spec-as-source."
```

## Workflow map (locked)

```text
Flow A: Design -> Implement -> Review -> Polish -> Land
1) design: create .design/ docs (intent + approach).
2) implement: read .design/*; update code; record divergences in .design/decisions.md.
3) review: read code + .design/*; write .design/review.md; do not delete design docs.
4) polish: rewrite the primary design doc to reflect current implementation.
5) land: remove .design/* contents, keep .design/ folder.

Flow B: Iteration across sessions
- Each session begins by reading .design/*.
- Each session ends by updating .design/decisions.md and the primary design doc if behavior changes.

Flow C: Fresh reviewer
- Reviewer treats .design/* as context, not a strict spec.
- Review focuses on quality with minor drift commentary.
```

## Implementation plan (locked)

```text
1) Migrate design doc location
   - Use .design/ instead of design/ across prompts and context.
   - Keep .design/ committed on branches.

2) Context loading
   - In src/loopflow/context.py, load .design/*.md into docs block.
   - Replace design/ loading with .design/ loading.

3) Review task
   - Update .claude/commands/review.md and src/loopflow/prompts/review.md:
     - Output to .design/review.md (filename not important).
     - Do not delete existing design docs.
     - Treat design docs as anchored context; minor drift commentary only.

4) Polish task
   - Update .claude/commands/polish.md and src/loopflow/prompts/polish.md:
     - Rewrite primary design doc to match implementation (overwrite).
     - Keep decisions log if present.

5) Landing cleanup
   - Update lf pr land and lf land to clear .design/* contents (keep folder).
```

## Done when

- .design/ is the only design-doc location; it is auto-included in context.
- Review writes .design/review.md and leaves design docs intact.
- Polish rewrites the primary design doc to match implementation.
- Landing removes .design/* contents without removing the folder.
- README/STYLE and prompts reflect the new workflow.

## Decisions (locked)

```text
- Use .design/ (not design/), committed on branches.
- Auto-include .design/ in all prompt contexts.
- Review writes .design/review.md (filename not important) and preserves existing docs.
- Polish overwrites primary design doc to match implementation.
- Landing removes .design/* contents, keeps folder.
- Review can mention drift lightly; prioritize face-value code assessment.
```
