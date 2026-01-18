---
layout: default
title: Philosophy
---

# Philosophy

Loopflow is a workflow layer for people who care about craft.

## The Loopflow Mindset

Engineers at scrappy AI labs, ML startups, and research-adjacent teams. High standards for craft. Skeptical of "just scale it" solutions. Want to work *with* agents—co-creation and delegation, not either/or.

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

## What We Reject

Not specific practices, but single-sided mentalities:

- **Pure throughput:** 10 parallel agents producing slop nobody reviews
- **Pure craft:** Refusing to use AI because it might make mistakes
- **Pure delegation:** "Let the agent figure it out" without quality gates
- **Pure control:** Micromanaging every token instead of trusting the workflow

We reject the false choice. We want the speed of automation *and* the quality of craft.

## The Hard Problems

AI agents can write code. The hard parts are:

**Context management.** Large codebases overwhelm agents. Compaction destroys important information. Without structure, agents miss context they need or drown in context they don't.

**Design intent.** New sessions start from scratch. The "why" behind decisions gets lost between sessions. Each conversation re-derives context that previous sessions already understood.

**Parallel work.** Running multiple agents means checking terminals, comparing outputs, deciding what to merge. Manual orchestration is distracting and error-prone.

**Quality.** Vibing produces slop. Without review passes and quality gates, code degrades.

## The Loopflow Approach

### Prompts as Artifacts

Prompts are markdown files in your repository. Versioned with git, reviewed in PRs, shared across your team. When something works, you can find it again.

```
.claude/commands/
  review.md
  implement.md
  design.md
```

### Chained Tasks

Chain tasks together with review steps between them. Each pass builds on the previous one.

```bash
lf implement && lf review && lf polish && lfops commit
```

### Backend-Agnostic

Same prompt runs on Claude, Codex, or Gemini. Switch with a flag:

```bash
lf review -m codex
lf implement --race claude,codex
```

You own your workflows. They survive when tools change.

### Worktrees for Isolation

Each feature gets its own directory. Agents work in isolated branches while you work on something else.

```bash
wt switch --create feature-a
lf ship &
```

## Craft and Throughput

Not either/or.

The goal isn't to remove humans from coding. It's to make the collaboration between humans and AI agents effective—structured, reproducible, and focused on craft.
