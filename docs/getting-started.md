---
layout: default
title: Get Started
---

# Get Started

## Quick fix

Copy an error, run one command. Context preloaded, normal Claude Code session.

```bash
lf debug -c
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

Run agents overnight, review PRs when you wake.

```bash
lfd loop ship src/
```

![loops demo](loops-demo.gif)

[Learn more →](agents.md)

---

## Install

```bash
uv tool install loopflow
lf init
```

Requires macOS. Run `lf init` to install Claude Code, worktrunk, and configure your repo.

## Try it

Clone the demo repo and fix a bug:

```bash
git clone https://github.com/loopflowstudio/loopflow-demos
cd loopflow-demos/calculator
python -m pytest test_calc.py    # see the bug
# copy error to clipboard
lf debug -c                       # fix it
```
