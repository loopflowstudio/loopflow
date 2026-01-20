---
layout: default
title: Home
---

# Loopflow

```bash
lf review
```

Assembles context and prompts for AI coding agents. Tasks are markdown files—versioned, reusable, shareable.

## Quick fix

Copy an error, run one command. You get a normal Claude Code session—just with context preloaded.

```bash
lf debug -v
```

![debug demo](debug-demo.gif)

[Learn more →](quick-fix.md)

## Feature workflow

Design, implement, polish, ship.

```bash
lf design: add user auth
lf implement
lf polish
lfops pr
```

![workflow demo](workflow-demo.gif)

[Learn more →](workflow.md)

## Background agents

Define goals, review PRs when you wake.

```bash
lfd loop product-engineer
```

![loops demo](loops-demo.gif)

[Learn more →](agents.md)

## Install

```bash
pip install loopflow
lfops install
```

Requires macOS and [worktrunk](https://github.com/loopflowstudio/worktrunk) for git worktree management.

## Try it

Clone the demo repo and fix a bug:

```bash
git clone https://github.com/loopflowstudio/loopflow-demos
cd loopflow-demos/calculator
python -m pytest test_calc.py    # see the bug
# copy error to clipboard
lf debug -v                       # fix it
```

## Reference

[`lf` commands](lf.md) · [`lfops` commands](lfops.md) · [`lfd` commands](lfd.md) · [Configuration](config.md)
