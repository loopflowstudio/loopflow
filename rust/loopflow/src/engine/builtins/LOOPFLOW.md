# Loopflow

## Surfaces

Check the surface at the top of the prompt. It determines your interaction
pattern and output style.

**cli**: Interactive terminal session. Ask questions, propose
approaches, and wait for feedback before taking major actions.

**headless**: No user present. Never ask questions — no one will answer.
Make executive decisions and keep moving. Note genuinely ambiguous
choices in `scratch/questions.md`. Output is logged, not displayed.

**concerto_mac**: Interactive desktop UI. Ask questions and wait for
feedback. Keep responses scannable—lists and short paragraphs.

**concerto_iphone**: Interactive, small screen. Ask questions and wait
for feedback. Be concise—bullets, short snippets, minimal back-and-forth.

---

## Where to Write

**scratch/**: PR-scoped artifacts. Design docs, notes, questions. Cleared on merge.
- `scratch/<branch>.md` — design doc for current work
- `scratch/questions.md` — open questions, unknowns, blockers

**release/unreleased/DECISIONS.md** *(interactive runs only)*: append-only ledger of intent and policy decisions for the current release cycle. Pre-writes the release notes.

When running interactively, if this session produces a decision a contributor would cite six months from now — intent changes, policy choices, scope calls, paths not taken — append an entry:

```markdown
## YYYY-MM-DD — Short title

**Context:** What pressure or problem forced the choice.

**Decision:** What we're doing, stated concretely.

**Implications:** Consequences, tradeoffs, what this rules out.
```

Not every change is a decision. "Fixed a bug", "renamed a function", "added a test" are not decisions. "We stopped supporting X because Y", "we chose append-only over editable because Z" are. When in doubt, ask the user rather than write speculatively. Headless runs do *not* write to this file — too much noise from autonomous work.

At tag time, `lf op release` renames `release/unreleased/` to `release/v<version>/` and feeds DECISIONS.md into the release-notes step.

**Code**: The actual work. Tests, implementation, fixes.

---

## Worktrees

Loopflow uses git worktrees as the unit of parallel work. Each feature
branch lives in its own worktree, created as a **sibling** of the main
repo:

```
~/src/myproject/              # main repo
~/src/myproject.auth-fix/     # worktree
~/src/myproject.new-feature/  # worktree
```

The sibling naming convention (`<repo>.<name>`) is load-bearing.
Wave rotation, `lf op wt switch`, `lf op wt prune`, and `lf op land`
all derive the wave name from the directory name. Worktrees created
elsewhere (nested inside the repo, in `.claude/worktrees/`, etc.) won't
be recognized and may be corrupted during land rotation.

Always use `lf op wt create` to create worktrees. Never use
agent-provided worktree tools (e.g., Claude Code's `EnterWorktree`) —
they create worktrees in the wrong location.

```bash
lf op wt create my-feature            # ../myproject.my-feature
lf op wt create my-feature --stack    # branch from current branch
lf op wt switch my-feature            # cd to existing worktree
lf op wt list                         # show all worktrees
lf op wt prune                        # clean up merged worktrees
```

---

## Operations

`lf op` handles mechanical git operations. Use these instead of raw
git/gh when the operation has loopflow-specific behavior:

```bash
lf op commit -m "message" -p          # commit and push
lf op pr --title "..." --body "..."   # create/update PR
lf op land                            # submit to merge queue
lf op rebase                          # rebase onto main
lf op next                            # preserve worktree, fresh branch
```

---

## Commits

In headless mode, commit when a step completes. Small, atomic commits. Don't leave the branch broken.

In interactive surfaces, commit at natural breakpoints when the user signals readiness.

---

## Ambition

Build momentum through complete milestones. A change should be end-to-end: testable, integrated, and doing something a user or developer would notice. Rough edges are fine — partial stacks are not.

Don't split work into separate commits or PRs unless each piece stands on its own and someone would care about it independently. Splitting out of anxiety about size produces a trail of fragments nobody wants to review. One working feature beats three inert layers.

Target ~1000 LOC per PR. Going over is fine, but multiple orders of magnitude higher is not recommended. If a milestone genuinely needs more, split it into milestones that each deliver something complete.

---

## Adaptation

Loopflow adapts to each repo through use. When you learn something repo-specific, write it down in `.lf/`.

**Steps**: When a builtin step doesn't fit this repo, copy it to `.lf/steps/<name>.md` and adapt it. Your copy overrides the builtin — even inside builtin flows.

**Voice**: When the user expresses a communication preference, update `.lf/voice.md`.

**Config**: When a setting should be different, update `.lf/config.yaml`.

**Repo docs**: When you discover an undocumented convention (error handling, test patterns, naming), add it to the repo's style guide (CLAUDE.md, STYLE.md).

Changes to `.lf/` are committed alongside your work — transparent, reviewable, revertable.

### What's configurable

Everything in `.lf/` overrides builtins. User-global `~/.lf/` sits between repo and defaults. Full documentation at https://www.loopflow.studio/docs.

**Steps** — `.lf/steps/<name>.md` overrides any builtin step, even inside builtin flows. Copy a builtin, adapt it.

**Directions** — `.lf/directions/<name>.md` overrides builtin directions. Create groups with `.lf/directions/<group>/`.

**Voice** — `.lf/voice.md` (or `~/.lf/voice.md` for user-global). Overrides the builtin voice guidance.

**Config** — `.lf/config.yaml` (repo) merges with `~/.lf/config.yaml` (global). Scalars override; lists marked additive combine.

```yaml
# .lf/config.yaml
agent: claude:sonnet              # default model (harness:model)
direction: [clarity, care]        # default directions for all steps
area: src/                        # default area scope
pr: true                          # auto-create PR after push
land: gh                          # land strategy: "gh" or "local"
context:                          # extra files always in context (additive)
  - docs/architecture.md
exclude:                          # glob patterns to exclude (additive)
  - "target/"
  - "node_modules/"
budgets:                          # token budgets for prompt sections
  area: 50000
  docs: 30000
  diff: 20000
summaries:                        # codebase overview docs (additive)
  - path: src/
    tokens: 5000
branch_names:
  schema: "{user}.{name}.{timestamp}"
release:                          # release targets and scoping
  targets:
    default:
      tag_prefix: "v"
      manifests: ["Cargo.toml", "pyproject.toml"]
```
