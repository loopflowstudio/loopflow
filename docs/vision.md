---
layout: default
title: Philosophy
---

# Philosophy

Loopflow is a workflow layer for people who care about craft.

## The Problem

AI agents can write code. The hard parts are context management, parallel orchestration, and quality control.

### Context Management

Large codebases overwhelm agents. Compaction destroys important information. Without structure, agents miss context they need or drown in context they don't.

### Design Intent

New sessions start from scratch. The "why" behind decisions gets lost between sessions and agents. Each conversation re-derives context that previous sessions already understood.

### Parallel Work

Running multiple agents means checking terminals, comparing outputs, deciding what to merge. Manual orchestration is distracting and error-prone.

### Quality

Vibing produces slop. Without review passes and quality gates, code degrades. Professional work needs discipline that pure automation can't provide.

## The Loopflow Approach

### Prompts as Artifacts

Prompts are markdown files in your repository. They're versioned with git, reviewed in PRs, and shared across your team. When something works, you can find it again. When something breaks, you can trace it back.

```
.claude/commands/
  review.md
  implement.md
  design.md
```

### Pipelines with Quality Gates

Chain tasks together with review steps between them. Each pass builds on the previous one.

```yaml
pipelines:
  ship:
    tasks: [implement, review, test, commit]
```

Design → Implement → Review → Ship. Quality gates ensure each stage meets standards before proceeding.

### Backend-Agnostic

Same prompt runs on Claude, Codex, or Gemini. Switch with a flag:

```bash
lf review -m codex
lf implement --parallel claude,codex
```

You own your workflows. They survive when tools change.

### Worktrees for Isolation

Each feature gets its own directory. Agents work in isolated branches while you work on something else. No conflicts, no context switching.

```bash
wt switch --create feature-a
lf ship &

# Meanwhile, you work elsewhere
cd ../myrepo
```

### Visual Orchestration

Maestro provides a visual interface for launching prompts, managing worktrees, and tracking sessions. See what's running, what's completed, and what needs attention.

## Target User

Loopflow is for engineers who want both speed and quality. Fast iteration *and* review passes. Autonomous agents *and* human oversight.

The "maestro" mindset: you shape the music while agents play the notes. In command, skilled, creating something good.

**What they value:**
- Craft *and* throughput
- Co-creation *and* delegation
- Focus *and* parallelism
- Understanding *and* shipping

**How it plays out:**
- Human in the loop, not out of it
- Unblock first, perfect later
- Prompts as artifacts, not chat logs
- Quality gates, not vibes

## What's Different

| Layer | Typical Pain | Loopflow Solution |
|-------|-------------|-------------------|
| Prompts | Scattered across tools | Markdown files in `.lf/`, versioned |
| Workflows | Locked to platforms | Portable pipelines, backend-agnostic |
| Orchestration | Terminal juggling | Session tracking, status dashboard |
| Quality | Ship and hope | Review tasks, pipelines with gates |
| Context | Lost between sessions | Persistent task files, design docs |
| Collaboration | Copy-paste prompts | Same `.lf/` folder for whole team |

## Tight Loops

Loops and flows are everywhere. Design → implement → learn. Build → observe → adjust.

Loopflow makes these loops tight and visible. Each pass teaches the next.

The goal isn't to remove humans from coding. It's to make the collaboration between humans and AI agents effective—structured, reproducible, and focused on craft.
