# Reframe: CLI-First Docs

## What to build

Rewrite docs around `lf` as a tight, focused tool — like worktrunk solves worktree management, `lf` solves **prompt and context construction**.

What's ready today: the CLI (`lf`, `lfops`). Pipelines, agents, and Maestro are coming next.

---

## The core problem `lf` solves

Prompt construction for coding agents:

- Assemble context (repo docs, diff, files, clipboard, summaries)
- Store prompts as reusable markdown files
- Pass it all to Claude, Codex, or Gemini

One problem, solved well.

---

## Tagline

**Keep:** "Arrange agents to code in harmony"

**Add subtitle:** "Reusable prompts. Composable workflows."

---

## Docs structure

```
docs/
  getting-started.md    # Install, run demo, see it work
  lf.md                 # Command reference (flags, :, args, context)
  builtins.md           # Built-in tasks: debug, design, implement, polish, review
  lfops.md              # Git workflow: pr, land, commit, init, install
  config.md             # .lf/config.yaml options
  patterns.md           # Recipes and tips

  next/
    pipelines.md        # Declarative task chaining
    agents.md           # lfd daemon, background agents
    maestro.md          # GUI app (the podium)
    multi-model.md      # Racing, parallel execution
    api.md              # Socket protocol
```

---

## Page outlines

### getting-started.md

1. What loopflow does: assembles context + prompt, passes to coding agent
2. Install: `pip install loopflow && lfops install`
3. Try it: clone demo repo, copy error, `lf debug -v`
4. How it works:
   - `lf` gathers context (repo docs, diff, files)
   - Adds your prompt (from a file or inline)
   - Passes everything to Claude/Codex/Gemini
5. Write your own prompts: `.claude/commands/my-task.md`
6. Ship with `lfops pr`
7. Next: `lf` reference, built-in tasks, config

### lf.md

- `lf <task>` — run a task file
- `lf <task>: args` — pass arguments
- `lf : "inline prompt"` — no task file
- Flags: `-i`, `-a`, `-x`, `-v`, `-c`, `-m`, `--voice`
- Task file locations: `.claude/commands/`, `.lf/`
- Context assembly: what gets included automatically
  - All `.md` files from repo root
  - Files touched by branch (with `--diff-files`)
  - Raw diff (with `--diff`)
  - Clipboard (with `-v`)
  - Extra files (with `-x`)

### builtins.md

- `debug` — paste error, fix it
- `design` — interactive spec writing
- `implement` — build from design doc
- `polish` — run tests, fix issues
- `review` — assess code quality
- `commit` — generate commit message

### lfops.md

- `lfops pr` — create/update PR, open in browser
- `lfops land` — squash-merge to main, cleanup worktree
- `lfops commit` — generate commit message and commit
- `lfops init` — scaffold `.lf/config.yaml`
- `lfops install` — install Claude Code, Codex, Gemini CLI

### config.md

- `agent_model` — default model (claude:opus, codex:o3, etc.)
- `context` — files always included
- `exclude` — patterns to skip
- `interactive` — tasks that default to interactive mode
- `voice` — default voice/persona
- `terminal` — for Maestro interactive sessions (coming soon)

### patterns.md

- Debug workflow: `lf debug -v`
- Design-first: `lf design` then `lf implement`
- Worktrees: use `wt` for branch isolation (link to worktrunk)
- Custom prompts: write your own task files

---

## Done when

```bash
# Main docs have no daemon/pipeline/Maestro references
grep -r "lfd\|daemon\|pipeline\|Maestro" docs/*.md  # empty

# next/ contains the coming-soon features
ls docs/next/  # pipelines.md, agents.md, maestro.md, etc.

# Getting started leads with what lf does
head -20 docs/getting-started.md | grep "context"
```
