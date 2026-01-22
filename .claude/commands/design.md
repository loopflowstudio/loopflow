---
interactive: true
produces: .design/<branch>.md
---
Produce a short implementation spec that another LLM session can use to write a first draft.

If on main, create a feature branch first: `git checkout -b <feature-name>`.

## Who reads this

The design doc is a working document for both humans and LLMs. The implementing session will execute fairly literally—what you don't specify, it will guess. But the human will likely read and edit directly before implementation. Optimize for easy to manipulate, not just easy to execute. Clear sections they can delete, add to, or rearrange. Constraints they can tighten or loosen.

The design doc is scaffolding—a checkpoint for recovery, not documentation for posterity. `lf ops pr land` deletes `.design/` contents.

## Workflow

1. Run `git branch --show-current` to confirm you're on a feature branch (not `main`)
2. Find the repository root with `git rev-parse --show-toplevel`
3. Create design doc at **repository root**: `<repo-root>/.design/<feature-name>.md` early—after the first exchange or two
4. Write as you go, refining with each conversation turn
5. Run `git add .design/ && git commit -m "design: <branch>"` when done
6. End session. Implementation happens via `lf implement`.

**Important:** The `.design/` directory must be at the repository root, not in subdirectories. If working on files in a subdirectory like `swift/`, still write design docs to the root `.design/` folder.

Write as you go, not at the end. If the session crashes mid-design, the partial doc is still useful. Let writing inspire questions—gaps become obvious when you make things concrete.

## What makes a good design doc

**Heavy on code.** Sketch data structures, function signatures, example API calls. The code is for communication, not execution:

```python
@dataclass
class Worktree:
    path: Path
    branch: str

def create_worktree(name: str) -> Worktree:
    """Create sibling worktree at ../{repo}.{name}"""
    ...
```

**Quote the user verbatim.** When they say something that captures intent, constraint, or priority—copy it into the doc. Quotes anchor what matters and prevent drift.

**Specify "done when."** A command to run, output to expect. The implementing session needs to know when to stop.

## Required sections (~1000 words max)

- **What to build** — One sentence. What exists after this that doesn't exist now.
- **Data structures** — Core types, sketched in code.
- **Key functions** — Signatures with one-line intent.
- **UI changes** — If adding config options or CLI flags, specify the Concerto UI updates. Skip only if explicitly infrastructure-only.
- **Constraints** — What would require rewriting if guessed wrong.
- **Done when** — Verification command and expected output. Include UI verification if applicable.

## Loopflow design principles

When designing for this codebase:

- **End-to-end by default.** Every feature that adds config options, CLI flags, or context sources must include Concerto UI updates. The Concerto app (under `swift/`) is the primary user interface—if users can't access a feature from the UI, it's incomplete. Infrastructure-only changes require explicit justification in the design doc (e.g., "Phase 1 of 2: backend only, UI in follow-up").
- **Worktrees are first-class.** Features should work across isolated worktrees. Don't assume a single working directory.
- **Prompts are files, not config.** Task definitions live in `.claude/commands/` or `.lf/`. Don't design template systems or prompt builders.
- **Design for auto mode.** Most tasks run headless. Don't require interactive confirmation for core flows.
- **Wrap, don't reimplement.** Loopflow delegates to Claude Code and Codex CLIs. Design to pass through, not duplicate.
- **Simple data structures.** Prefer `@dataclass` over inheritance hierarchies. Prefer functions over classes when state isn't needed.

## Conversation guidance

Bias toward brevity. Ask only what's needed to start coding.

If scope is unclear, ask "what's the smallest version that's useful?" and spec that.

Completeness is not required. Wrong guesses get fixed in implementation. The goal is to not block the implementing session, not to predict everything.

