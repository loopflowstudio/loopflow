---
layout: default
title: Feature Workflow
---

# Feature Workflow

Design, implement, polish, ship. Each step reads what the previous one wrote.

![workflow demo](workflow-demo.gif)

## The flow

```bash
lfops wt create auth-feature       # create worktree with schema-based branch
lf design: add OAuth login         # interactive: discuss approach
lf implement                       # builds what you designed
lf polish                          # runs tests, fixes issues
lfops pr                           # open PR
```

## Worktrees

Each feature gets its own directory. No branch switching, no stashing.

```bash
lfops wt create my-feature       # create worktree with schema-based branch
lfops wt prune                   # cleanup merged worktrees
```

Requires [worktrunk](https://github.com/loopflowstudio/worktrunk) for underlying worktree operations.

## Built-in steps

### design

```bash
lf design: add OAuth login
```

Interactive session to explore the problem. Writes spec to `.design/<branch>.md`.

### implement

```bash
lf implement
```

Reads `.design/<branch>.md` and builds it. Runs in auto mode by default.

### lint

```bash
lf lint
```

Runs ruff checks, fixes issues. Called automatically by `lfops land`.

### polish

```bash
lf polish
```

Runs tests, fixes failures. Keeps going until tests pass.

### review

```bash
lf review
```

Assesses code quality. Fixes issues it finds.

## How steps chain

Tasks pass state through files:

| Task | Reads | Writes |
|------|-------|--------|
| design | — | `.design/<branch>.md` |
| implement | `.design/<branch>.md` | code |
| polish | code, tests | code |
| review | code | `.design/review.md` |

The `.design/` folder is your PR scratchpad—cleared when the PR merges.

## Shipping

```bash
lfops pr      # create or update PR
lfops land    # submit to merge queue
```

`pr` is idempotent—run it to create, or again after more commits. `land` enables auto-merge; GitHub merges when CI passes.

## Custom steps

Tasks are markdown files in `.claude/commands/`:

```bash
lf test    # runs .claude/commands/test.md
```

Create your own. Override built-ins by using the same name.

## External skills

Use skills from external libraries alongside your own:

```bash
lf sp:brainstorm: add user auth    # superpowers brainstorm
lf sr:gog                          # SkillRegistry skill
lf implement                        # loopflow implement
lf sp:execute-plan                  # superpowers execution
```

If `~/.superpowers` exists, it's auto-detected. SkillRegistry is opt-in via config. See [Configuration](config.md#skill-sources).

## Parallel features

Run multiple features in separate worktrees:

```bash
# Terminal 1
lfops wt create feature-a
lf implement: add caching

# Terminal 2
lfops wt create feature-b
lf implement: add auth
```

## Next

Ready for autonomous agents? [Background agents →](agents.md)
