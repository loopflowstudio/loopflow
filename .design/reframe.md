# Reframe: CLI-First Docs

## What to build

Rewrite docs for someone installing loopflow CLI today. No agents, no Maestro, no pipelines in the main flow. Those go in "under development."

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

  under-development/
    pipelines.md        # Declarative task chaining
    agents.md           # lfd daemon, background agents
    maestro.md          # GUI app
    multi-model.md      # Racing, parallel execution
    api.md              # Socket protocol
```

---

## Page outlines

### getting-started.md

1. Install: `pip install loopflow && lfops install`
2. Try it: clone demo repo, copy error, `lf debug -v`
3. Full workflow:
   ```bash
   wt switch --create my-feature
   lf design: add auth
   lf implement
   lf polish
   lfops pr
   ```
4. Next: read about `lf` command, built-in tasks, or `lfops`

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

---

## Done when

```bash
# Main docs have no daemon/pipeline/Maestro references
grep -r "lfd\|daemon\|pipeline\|Maestro" docs/*.md  # empty

# Under-development contains the advanced features
ls docs/under-development/  # pipelines.md, agents.md, maestro.md, etc.

# Getting started shows the demo workflow
grep "lf debug -v" docs/getting-started.md
```
