---
layout: default
title: Quick Fix
---

# Quick Fix

One command. Context preloaded. Normal Claude Code session.

![debug demo](debug-demo.gif)

## How it works

```bash
lf debug -v
```

1. `lf` assembles context—repo docs, branch files, clipboard
2. Passes everything to Claude Code
3. You get an interactive session with context already loaded

The `-v` flag pastes your clipboard. Copy an error, run the command, watch it fix.

## What's included

```
Tokens: 8,247

files          4,892 ████
  STYLE.md     1,854 █
  README.md      588 ▏
diff_files     2,721 ██
  src/calc.py  1,847 █
  tests/         874 ▏
clipboard        634 ▏
```

Run `lf debug -c` to see the breakdown without launching.

## Inline prompts

Skip the task file entirely:

```bash
lf : "fix the typo in README"
lf : "add type hints to utils.py"
lf : "rename getUserById to findUserById everywhere"
```

## Context flags

| Flag | What it adds |
|------|--------------|
| `-v` | Clipboard content |
| `-x FILE` | Specific file or directory |
| `--diff` | Raw `git diff` output |
| `--no-lfdocs` | Skip repo docs |

```bash
lf debug -v                           # paste clipboard
lf : "explain this" -x src/auth.py    # add specific file
```

## Different models

```bash
lf debug -v -m codex        # use Codex instead
lf debug -v -m gemini       # use Gemini
lf debug -v -m claude:opus  # use Claude Opus
```

Same context assembly, different backend.

## Auto mode

Add `-a` to run without interaction—agent works to completion:

```bash
lf debug -v -a    # auto mode: fixes and stops
```

Default is interactive. Auto mode is useful for chaining commands.

## Next

Ready to build features? [Feature workflow →](workflow.md)
