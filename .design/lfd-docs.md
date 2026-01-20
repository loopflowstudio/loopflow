# Documentation Restructuring

## What to build

Restructure loopflow docs to match industry patterns, add missing `lfd.md` command reference, and expand the README.

## Research Summary

Analyzed docs from Stripe, OpenAI, Letta, Conductor.build, LangChain, and Anthropic.

**Common structure across leaders:**

| Section | Purpose | Examples |
|---------|---------|----------|
| **Get Started** | First 5 minutes | Quickstart, installation |
| **Concepts** | Mental model | Core ideas, architecture |
| **Guides** | Task-oriented | How to do X |
| **Reference** | Exhaustive lookup | API/CLI reference |
| **Tutorials** | Complete examples | Cookbook, templates |
| **Troubleshooting** | Common issues | FAQ, tips |
| **Changelog** | What's new | Release notes |

**Notable patterns:**
- Stripe: Use-case navigation ("Accept payments" not "Payments API")
- Conductor.build: Quickstart templates for frameworks (NextJS, Rails, Django)
- LangChain: Parallel Python/TypeScript structure
- Letta: Progressive depth within topics
- Anthropic: Card-based homepage, separate Cookbook

## Current Loopflow Docs

```
docs/
  index.md          # homepage
  getting-started.md
  lf.md             # reference ✓
  lfops.md          # reference ✓
  builtins.md       # guides (tasks)
  config.md         # reference
  patterns.md       # how-to recipes ✓
  storage.md        # concepts
  loops.md          # concepts + commands mixed
  triggers.md       # concepts + commands mixed
```

## Gap Analysis

| Gap | Impact | Fix |
|-----|--------|-----|
| No `lfd.md` | Users can't look up lfd flags quickly | Add command reference |
| loops.md mixes concepts + commands | Hard to scan for CLI usage | Extract commands to lfd.md |
| No explicit Concepts section | Mental model scattered | Rename storage.md, group conceptual content |
| No Troubleshooting/FAQ | Users stuck on common issues | Add troubleshooting.md |
| Changelog not in docs nav | Users miss updates | Link RELEASE_NOTES.md or add changelog.md |
| README doesn't reflect docs growth | New users miss docs | Expand README docs section |

## Proposed Structure

```
docs/
  index.md              # homepage (keep)
  getting-started.md    # quickstart (keep)

  # Concepts
  concepts.md           # NEW: mental model overview
  storage.md            # where files live (rename to concepts/storage.md or keep)

  # Guides
  builtins.md           # built-in tasks (keep)
  patterns.md           # workflows (keep)
  loops.md              # background agents guide (trim CLI details)
  triggers.md           # subscribe/schedule guide (trim CLI details)

  # Reference
  lf.md                 # lf CLI (keep)
  lfops.md              # lfops CLI (keep)
  lfd.md                # NEW: lfd CLI reference
  config.md             # configuration (keep)

  # Support
  troubleshooting.md    # NEW: FAQ, common issues
  changelog.md          # NEW: link to or mirror RELEASE_NOTES.md
```

## Priority Order

1. **lfd.md** — Missing reference, blocks users
2. **README expansion** — First thing people see
3. **troubleshooting.md** — High value, low effort
4. **concepts.md** — Improves discoverability
5. **changelog.md** — Nice to have

---

## Part 1: lfd.md Command Reference

### Commands to Document

| Command | Purpose |
|---------|---------|
| `lfd serve` | Run daemon in foreground |
| `lfd install` | Install launchd service |
| `lfd uninstall` | Remove launchd service |
| `lfd start` | Start multiple loops |
| `lfd loop` | Start continuous loop |
| `lfd flow` | Run single iteration |
| `lfd subscribe` | Watch paths on main |
| `lfd schedule` | Run on cron schedule |
| `lfd status` | Show loop status |
| `lfd stop` | Stop a running loop |
| `lfd prs` | Show PRs from a loop |
| `lfd rm` | Remove loop and history |
| `lfd list-goals` | Show available goals |

### Flags Reference

```
lfd loop <goal>
  -a, --area        Area override (pathset)
  -l, --limit       PR limit override
  --merge-mode      pr | land
  -f, --foreground  Run in foreground

lfd flow <goal>
  -p, --project     Project/prompt file
  -v, --paste       Include clipboard
  -r                Area override

lfd subscribe <pathset> <goal>
  -r                Area override

lfd schedule "<cron>" <goal>
  -p, --project     Project file
  -r                Area override

lfd start [goals...]
  -a, --all         Include waiting loops

lfd stop <loop-id>
  -f, --force       Force kill (SIGKILL)

lfd prs <loop-id>
  -n, --limit       Number to show (default: 10)

lfd rm <loop-id>
  -f, --force       Skip confirmation
```

---

## Part 2: README Expansion

Current README docs section:

```markdown
## Documentation

- [Getting Started](docs/getting-started.md)
- [Built-in Tasks](docs/builtins.md)
- [Configuration](docs/config.md)
- [Patterns](docs/patterns.md)
- [Command Reference](docs/lf.md)
```

Proposed expansion:

```markdown
## Documentation

**Get started:**
- [Getting Started](docs/getting-started.md) — install, first task, ship

**Guides:**
- [Built-in Tasks](docs/builtins.md) — debug, design, implement, polish, review
- [Patterns](docs/patterns.md) — workflows and recipes
- [Loops](docs/loops.md) — background agents
- [Triggers](docs/triggers.md) — subscribe and schedule

**Reference:**
- [`lf`](docs/lf.md) — run tasks
- [`lfops`](docs/lfops.md) — git workflow
- [`lfd`](docs/lfd.md) — daemon commands
- [Configuration](docs/config.md) — .lf/config.yaml
- [File Storage](docs/storage.md) — where things live
```

---

## Part 3: troubleshooting.md (Sketch)

```markdown
# Troubleshooting

Common issues and solutions.

## lfd daemon not running

Check if installed:
\`\`\`bash
launchctl list | grep lfd
\`\`\`

Reinstall:
\`\`\`bash
lfd install
\`\`\`

## Task hangs in auto mode

Check if waiting for input. Use `-i` for interactive mode if the task needs clarification.

## Rate limits

Hit Claude/Codex rate limits? Reduce parallel agents or wait.

## Worktree issues

\`\`\`bash
wt list              # see all worktrees
git worktree prune   # clean up stale entries
\`\`\`

## See Also

- [Patterns](patterns.md) — workflows
- [Configuration](config.md) — options
```

---

## Constraints

- Match existing doc style
- Don't restructure folders yet (keep flat docs/)
- Link between docs, don't duplicate
- Keep it scannable: tables over prose

## Done When

1. `docs/lfd.md` exists with all 13 commands documented
2. README.md has expanded documentation section
3. `docs/troubleshooting.md` exists with common issues
4. All internal links work
