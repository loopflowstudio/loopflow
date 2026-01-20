---
layout: default
title: File Storage
---

# File Storage

Where loopflow stores things, and why.

## The Core Idea

Loopflow treats **prompts as artifacts**. Not chat logs. Not clipboard snippets. Files in your repo—versioned with git, reviewed in PRs, shared across your team.

This extends to everything loopflow touches:

- **Tasks** live in `.claude/commands/`
- **Config** lives in `.lf/`
- **Working state** lives in `.design/`
- **Internal docs** live in `.docs/`

When something works, you can find it again. When something breaks, you can trace it back.

## Folder Reference

```
.claude/commands/     # Task prompts
.lf/                  # Config and extensions
.design/              # Current PR working state
.docs/                # Internal documentation
docs/                 # Public documentation
```

### `.claude/commands/` — Task Prompts

The primary home for task definitions.

```
.claude/commands/
  review.md
  implement.md
  debug.md
```

**Why here?** Claude Code compatibility. Tasks in `.claude/commands/` work with both `lf` and native Claude Code slash commands. Portable across tools.

**What goes here:** Single-purpose prompts. Each file is one task—review, implement, debug, polish. The filename becomes the command: `lf review` runs `review.md`.

**What doesn't:** Config, personas, long-running goals. Those belong in `.lf/`.

### `.lf/` — Config and Extensions

Everything loopflow-specific that isn't a task prompt.

```
.lf/
  config.yaml         # Repo configuration
  voices/             # Personas for agent responses
    concise.md
    architect.md
  goals/              # Directives for agent loops
    test-coverage.md
  summaries/          # Generated codebase summaries
```

**`config.yaml`** — Model selection, pipelines, context defaults. See [Configuration](config.md).

**`voices/`** — Personas that shape how agents respond. "Be concise." "Think like an architect." Applied with `--voice`.

**`goals/`** — High-level directives for autonomous agent loops. Unlike tasks (single-purpose), goals describe ongoing objectives an agent works toward across multiple iterations. Used by background agents via `lfd`.

**`summaries/`** — Pre-generated codebase overviews for large repos. Created with `lfops summarize`. Gitignored—regenerated as needed.

### `.design/` vs `.docs/` — The Key Distinction

These two folders serve different purposes with different lifespans:

| | `.design/` | `.docs/` |
|---|---|---|
| **Lifespan** | Dies with the PR | Lives forever |
| **Purpose** | Current work scratchpad | Forward-looking plans |
| **In context?** | Yes | Yes |
| **Cleared on merge?** | **Yes** | No |
| **Example** | "Add auth to user endpoint" spec | "How auth should work long-term" |

### `.design/` — Current PR Only

Ephemeral scratchpad for the current branch. Committed to git, visible in the PR, but **deleted when the PR merges**.

```
.design/
  feature-name.md     # Design spec for this PR
  questions.md        # Open questions from this work
  review.md           # Review verdict
```

**What goes here:** Anything that helps *this PR* but shouldn't persist. Design specs, captured questions, review notes. By merge time, the code speaks for itself.

**The rule:** If it matters after merge, put it in `.docs/` instead.

### `.docs/` — Persists Forever

Forward-looking internal documentation. What we're building next, not what we've already built.

```
.docs/
  vision.md           # Where the product is heading
  roadmap/            # What's coming next
  research/           # Background for future work
```

**What goes here:** Plans for things not yet coded. Product direction. Research that informs future decisions.

**What doesn't go here:** "Why we built X this way"—that belongs in a README next to the code, or in code comments.

**This evolves.** Agents write most of it. Humans refine and redirect.

### `.docs/` vs `docs/` — Internal vs Public

| | `.docs/` | `docs/` |
|---|---|---|
| **Audience** | Maintainers | Users |
| **Focus** | Forward-looking, not yet built | Backwards-looking, what exists |
| **In context?** | Yes | No (opt-in) |
| **Who writes** | Mostly agents, humans edit | Humans |
| **Example** | "How auth should work" | "How to authenticate requests" |

### `docs/` — Public Documentation

Backwards-looking, public-facing documentation. What exists today, how to use it.

```
docs/
  getting-started.md
  config.md
  patterns.md
```

**Audience:** People using your software. External users, new team members.

**Backwards-looking:** Documents what's already built. Updated when the product changes. Polished, reviewed, human-maintained.

## The Philosophy

Loopflow's file storage follows from its core beliefs:

### Documentation for Humans and Agents

Good documentation serves both audiences. Loopflow puts context where it naturally belongs—not in special agent-only files, but in places humans already look and edit.

- **`.docs/`** — internal docs that humans and agents both read and write
- **`.design/`** — specs humans review, agents implement from, either can edit
- **`docs/`** — public docs humans maintain, agents reference

When you write a design doc, you're writing for the implementing agent *and* for the human who might tweak it before running `lf implement`. When agents add notes to `.docs/`, they're building knowledge you can read and refine directly—no special viewer needed, just markdown in your repo.

Run `lf -c` to preview exactly what context gets assembled.

### Other Principles

**Prompts as artifacts.** If it's worth running, it's worth saving. Tasks live in the repo, not your clipboard.

**Human in the loop.** Everything is readable, editable, traceable. No magic databases. No hidden state. Files you can `git diff`.

**Unblock first, perfect later.** `.design/` is messy by design. Capture questions, write drafts, iterate. Polish happens later. The ephemeral nature prevents perfectionism—you know it'll be deleted.

**Craft over vibes.** `.docs/decisions/` captures why. Review verdicts go in `.design/`. The structure enforces discipline without requiring it in every prompt.

## Quick Reference

| Location | Purpose | Persists? | In context? |
|----------|---------|-----------|-------------|
| `README.md`, `STYLE.md`, etc. | Top-level guidance | Yes | **Yes** |
| `.design/` | PR scratchpad | **No** (cleared) | **Yes** |
| `.docs/` | Forward-looking plans | Yes | **Yes** |
| `docs/` | Public docs | Yes | No (opt-in) |
| `.claude/commands/` | Task prompts | Yes | (task file only) |
| `.lf/config.yaml` | Repo config | Yes | No |
| `.lf/voices/` | Personas | Yes | (when `--voice`) |
| `.lf/goals/` | Agent directives | Yes | (when agent uses) |
| `.lf/summaries/` | Summaries | Gitignored | (when configured) |

**Auto-included:** Well-known guidance files (`README.md`, `STYLE.md`, `CLAUDE.md`, `AGENTS.md`), `.design/`, and `.docs/`. Not every `.md` file—`docs/` is opt-in.

## See Also

[Configuration](config.md) · [Built-in Tasks](builtins.md) · [Patterns](patterns.md)
