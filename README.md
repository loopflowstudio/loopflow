# Loopflow

```bash
lf review
```

Assembles context and prompts for AI coding agents. Tasks are markdown files—versioned, reusable, shareable.

## Quick fix

Copy an error, watch it fix.

```bash
lf debug -v
```

![debug demo](docs/debug-demo.gif)

## Feature workflow

Design, implement, polish, ship.

```bash
lf design: add user auth
lf implement
lf polish
lfops pr
```

![workflow demo](docs/workflow-demo.gif)

## Background agents

Define goals, review PRs when you wake.

```bash
lfd loop product-engineer
```

![loops demo](docs/loops-demo.gif)

## Install

```bash
pip install loopflow
lfops install
```

Requires macOS and [worktrunk](https://github.com/loopflowstudio/worktrunk) for git worktree management.

## Documentation

[Read the docs →](docs/index.md)

## License

MIT
