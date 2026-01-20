---
layout: default
title: Patterns
---

# Patterns

Workflows and recipes for common scenarios.

## Quick Debug

Copy an error, paste it to loopflow:

```bash
# Run something, get an error, copy it
lf debug -v    # -v pastes clipboard
```

The debug task reads the stacktrace, finds the file, fixes the bug. No context-gathering required.

## Design-First Development

Start with a design task that explores the problem before writing code.

```bash
wt switch --create auth-feature
lf design: add OAuth login    # interactive: discuss approach
lf implement                  # builds what you designed
lf polish                     # runs tests, fixes issues
lf review                     # produces verdict
```

The built-in design task writes to `.design/<branch>.md`. Implement reads it automatically.

## Parallel Features

Run multiple features simultaneously in separate worktrees:

```bash
# Terminal 1
wt switch --create feature-a
lf implement: add caching &

# Terminal 2
wt switch --create feature-b
lf implement: add auth &

# Check status
lfops status
```

## Summarization

For large codebases, pre-generate summaries:

```bash
lfops summarize src/    # generate summary for src/
```

Configure paths in `.lf/config.yaml`. See [Configuration](config.md).

## Context Options

```bash
lf implement -x src/models.py -x src/api.py: add user endpoints
lf debug -v    # -v pastes clipboard
```

Set defaults in `.lf/config.yaml`. See [Configuration](config.md).

## Inline Prompts

Quick one-off tasks without a task file:

```bash
lf : "fix the typo in the README"
lf : "add type hints to utils.py"
lf : "rename getUserById to findUserById everywhere"
```

## PR Workflow

```bash
lfops pr      # create or update PR (CI runs automatically)
lfops land    # submit to merge queue
```

The `pr` command is idempotent: run it to create, or again to update after more commits. The `land` command enables auto-merge—GitHub merges when CI passes.

## Full Feature Workflow

A typical workflow from start to ship:

```bash
wt switch --create my-feature       # create worktree
lf design: describe the feature     # interactive design session
lf implement                        # build from design
lf polish                           # run tests, fix issues
lf review                           # final quality check
lfops commit                        # commit with generated message
lfops pr                            # create PR (CI runs)
lfops land                          # submit to merge queue
wt remove my-feature                # cleanup after merge
```

## Different Models

Override the default model per-task:

```bash
lf review -m codex              # use Codex
lf implement -m gemini          # use Gemini
lf debug -m claude:opus         # use Claude Opus
```

## Voices

```bash
lf review --voice concise
lf implement --voice architect,concise  # multiple voices
```

Create voices as markdown files in `.lf/voices/`.

## Custom Tasks

```bash
lf test    # runs .claude/commands/test.md
```

Create your own tasks in `.claude/commands/`.

## Worktrees

Use git worktrees to keep features isolated:

```bash
wt switch --create feature       # create worktree
wt list                          # show all worktrees
wt remove feature                # delete worktree + branch
```

See [worktrunk](https://github.com/loopflowstudio/worktrunk) for the full `wt` command reference.

## Mixing Loopflow and External Skills

Use your own tasks alongside skills from external libraries like [superpowers](https://github.com/obra/superpowers):

```bash
wt switch --create my-feature
lf sp:brainstorm: add user auth   # use superpowers brainstorm skill
lf implement                       # use loopflow implement task
lf sp:execute-plan                 # use superpowers execution skill
lf review                          # use loopflow review task
```

External skills get loopflow's context assembly. If `~/.superpowers` exists, it's auto-detected with prefix `sp`.

```bash
lf --list    # see all available tasks and skills
```
