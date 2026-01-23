# Loopflow

Loopflow runs prompts against LLM coding agents (Claude Code, Codex, Gemini CLI). Prompts are single-purpose and chain together—each one inherits state from the previous step and leaves state for the next.

Principle: tight loops. Do one thing, hand off cleanly.

---

## Run Modes

**Auto mode** (default): Non-interactive. The prompt runs to completion without user input. Output streams to the terminal and logs to `~/.lf/logs/`.

**Interactive mode** (`-i` flag): Full chat interface. The session can be interrupted, redirected, or extended. Ctrl+C kills the session immediately with no cleanup.

---

## Context

Prompts receive context assembled by loopflow. What's included is configurable via `.lf/config.yaml`. Typical context:

- Root-level `.md` files (README, STYLE, etc.)
- Current diff against main
- The step prompt from `.claude/commands/<step>.md` or `.lf/<step>.md`
- Additional files via `-x` flag or `context:` in config

---

## Commits

In pipelines, loopflow commits between steps automatically.

In interactive mode, commit at natural breakpoints. Don't leave the branch in a broken state.

---

## File Structure

```
.lf/
  config.yaml      # repo configuration
  steps/           # step prompts
  voices/          # personas
  flows/           # flow definitions

.claude/commands/  # step prompts (Claude Code compatible)

scratch/           # PR scratchpad (root only, cleared on merge)
  <branch>.md      # design doc for current branch
  questions.md     # open questions captured during runs

roadmap/           # internal docs (persists)
  vision.md        # where we're going
  <area>/          # area-specific plans
```

**scratch/** is for the current PR. Keep `scratch/<branch>.md` current as work progresses—what's done, what's left, what changed. Open questions go in `scratch/questions.md`. Don't block on unknowns; capture them and keep moving. Cleared automatically when the PR merges.

**roadmap/** is for forward-looking plans that persist. Root `roadmap/` holds cross-cutting docs. Per-folder `roadmap/` (like `src/api/roadmap/`) holds area-specific plans. Including a path auto-includes its nested roadmap folders.

---

## Worktrees

Loopflow works best with git worktrees. Each feature gets its own directory, isolated from other work.

```bash
lfops wt create <name>         # create worktree with schema-based branch
lfops wt prune                 # remove merged worktrees
```

Most prompts expect to run on a feature branch, not main. If a branch doesn't exist yet, create a worktree.

---

## Example: How Prompts Chain

Prompts can require and produce specific artifacts. For example:

| Prompt | Requires | Produces |
|--------|----------|----------|
| design | — | scratch/<branch>.md |
| implement | scratch/<branch>.md | code, tests |
| polish | code on branch | passing tests |
| review | code on branch | verdict in scratch/ |

Common sequences:

```
design → implement → polish
```
Design writes a spec. Implement builds it. Polish verifies tests pass.

```
review → iterate → polish
```
Review finds issues. Iterate fixes them. Polish closes it out.

These are examples. Check `.lf/flows/` for the actual pipelines configured in this repo.

---

## Auto Mode Behavior

In headless execution, prompts run without interactive input. Make best-effort assumptions and keep moving. Questions get captured in `scratch/questions.md` for the next pass.
