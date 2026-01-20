---
layout: default
title: Built-in Tasks
---

# Built-in Tasks

Loopflow includes these tasks out of the box. Override any by creating your own in `.claude/commands/`.

## debug

```bash
lf debug -v    # paste error, fix it
```

## design

```bash
lf design: add OAuth login    # interactive spec writing
```

Creates `.design/<branch>.md`. The implement task reads this automatically.

## implement

```bash
lf implement    # build from .design/<branch>.md
```

## polish

```bash
lf polish    # run tests, fix failures
```

## review

```bash
lf review    # assess code quality, fix issues
```

## commit

```bash
lf commit    # generate commit message from diff
```

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

Create `.claude/commands/review.md` and your version takes precedence.

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

[`lf` reference](lf.md) · [Configuration](config.md) · [Patterns](patterns.md)
