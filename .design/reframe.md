# Reframe: CLI-First Docs

## What to build

Rewrite docs around `lf` as a tight, focused tool — like worktrunk solves worktree management, `lf` solves **prompt and context construction**.

What's ready today: the CLI (`lf`, `lfops`, `wt`). Pipelines, agents, and Maestro are coming soon.

---

## The core problem `lf` solves

Prompt construction for coding agents:
- Assemble context (repo docs, diff, files, clipboard)
- Store prompts as reusable files
- Pass it all to Claude/Codex/Gemini

That's it. One problem, solved well.

---

## Tagline

**Keep:** "Arrange agents to code in harmony"

**Add subtitle:** "Reusable prompts. Composable workflows."

---

## Docs structure

```
docs/
  getting-started.md    # Install, run demo, see it work
  lf.md                 # lf command reference (flags, :, args)
  builtins.md           # Built-in tasks: debug, design, implement, polish, review
  lfops.md              # lfops pr, land, commit, init, install
  config.md             # .lf/config.yaml options

  next/
    pipelines.md        # Declarative task chaining
    agents.md           # lfd daemon, background agents
    maestro.md          # GUI app
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
- Context: what gets included automatically

### builtins.md

- `debug` — paste error, fix it
- `design` — interactive spec writing
- `implement` — build from design doc
- `polish` — run tests, fix issues
- `review` — assess code quality
- `commit` — generate commit message

### lfops.md

- `lfops pr` — create/update PR
- `lfops land` — squash-merge, cleanup worktree
- `lfops commit` — generate commit message and commit
- `lfops init` — scaffold config
- `lfops install` — install dependencies

### config.md

- `agent_model` — default model
- `context` — files always included
- `exclude` — patterns to skip
- `interactive` — tasks that default to interactive
- `terminal` — for Maestro (under development)
- `voice` — default voice

### patterns.md (or tips section)

- Worktrees: mention `wt` for branch isolation, link to worktrunk
- Debug workflow: `lf debug -v`
- Design-first: `lf design` then `lf implement`

---

## Done when

```bash
# Main docs have no daemon/pipeline/Maestro references
grep -r "lfd\|daemon\|pipeline\|Maestro" docs/*.md  # empty

# next/ contains the advanced features
ls docs/next/  # pipelines.md, agents.md, maestro.md, etc.

# Getting started shows the demo workflow
grep "lf debug -v" docs/getting-started.md
```
