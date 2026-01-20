---
layout: default
title: Built-in Tasks
---

# Built-in Tasks

Loopflow includes these tasks out of the box. Override any by creating your own in `.claude/commands/`.

## debug

Paste an error, fix it.

```bash
# Copy an error to clipboard, then:
lf debug -v
```

The debug task reads the stacktrace, finds the file, and fixes the bug. Best used with `-v` to paste clipboard content.

**Default mode:** auto

## design

Interactive spec writing.

```bash
lf design: add OAuth login
```

Creates `.design/<branch>.md` with the design spec. Use this to explore the problem before writing code. The implement task reads this file automatically.

**Default mode:** interactive

## implement

Build from a design doc.

```bash
lf implement
```

Reads `.design/<branch>.md` and implements what's described there. Works best after running `lf design`.

**Default mode:** auto

## polish

Run tests, fix issues.

```bash
lf polish
```

Runs the test suite, fixes any failures, and cleans up rough edges. Use after implement to ensure everything works.

**Default mode:** auto

## review

Assess code quality.

```bash
lf review
```

Reviews the diff on the current branch. Looks for bugs, edge cases, style violations, and missing tests. Fixes issues directly rather than just reporting them.

**Default mode:** auto

## commit

Generate a commit message.

```bash
lf commit
```

Stages changes and generates a commit message based on the diff. Used internally by `lfops commit`.

**Default mode:** auto

## Workflow Example

A typical workflow chains these tasks:

```bash
wt switch --create auth-feature
lf design: add OAuth login    # interactive: discuss approach
lf implement                  # builds what you designed
lf polish                     # runs tests, fixes issues
lf review                     # final quality check
lfops pr                      # open PR
```

## Overriding Built-ins

Create your own version in `.claude/commands/`:

```markdown
# .claude/commands/review.md

Review the diff on this branch.

## My team's rules
- All functions must have type hints
- Tests required for new features
- No TODO comments in production code
```

Your version takes precedence over the built-in.

## External Skills

Beyond built-ins, loopflow can run skills from external libraries. External skills use a `prefix:name` format:

```bash
lf sp:brainstorm              # run brainstorm from superpowers
lf sp:write-plan              # run write-plan from superpowers
```

External skills get loopflow's full context assembly (docs, diff, branch files) even though they're defined elsewhere.

To see all available tasks including external skills:

```bash
lf --list
```

Configure skill sources in `.lf/config.yaml`. See [Configuration](config.md#skill-sources) for details.

## See Also

- [`lf` command reference](lf.md) — flags and options
- [Configuration](config.md) — `.lf/config.yaml` options
- [Patterns](patterns.md) — workflows and recipes
