# Loopflow Vision

Arrange agents to code in harmony. Reusable prompts. Composable workflows.

---

## Software as Craft

Writing software is like writing music. Yes, it produces something valuable. But we also believe in software as an art—a craft that has merit as an experience for the creator.

Loopflow is for **softwarists**: software artists who love making software and want to reach their fullest potential with AI support. Not just shipping faster, but creating better. The joy is in the making.

---

## What Loopflow Is

A tight, focused tool for **prompt and context construction**. Like worktrunk solves worktree management, `lf` solves assembling context and prompts for coding agents.

- Gather context (repo docs, diff, files, clipboard)
- Store prompts as reusable markdown files
- Pass it all to Claude, Codex, or Gemini

One problem, solved well.

---

## What's Ready Today

**The CLI:**

```bash
lf debug -v              # paste error, watch it fix
lf design: add auth      # interactive design session
lf implement             # build from design
lf polish                # run tests, fix issues
lfops pr                 # open PR
lfops land               # squash-merge, cleanup
```

Tasks are markdown files in `.claude/commands/` or `.lf/`. Prompts are artifacts—versioned with git, reviewed in PRs, shared across your team.

---

## What's Coming Next

**Pipelines** — declarative task chaining with auto-commit between steps

**Background agents** — `lfd` daemon for autonomous work while you sleep

**Concerto** — the podium (see below)

**Multi-model** — race Claude vs Codex, pick the winner

---

## Concerto: The Podium

Concerto is where you conduct from—launch tasks, watch progress, ship when ready. It's not a terminal replacement. Interactive sessions happen in your terminal; Concerto shows everything and directs the work.

**Design principle:** Play nice with existing tools. Don't compete with terminals.

### What Concerto Does

- See all worktrees and their status
- Stream live output from running auto tasks
- Launch interactive sessions in your terminal (Warp, Ghostty, etc.)
- Ship with one click: PR, land, cleanup

### What Concerto Doesn't Do

- No embedded terminal
- No competing with your IDE
- No replacing tools you already like

---

## The Core Problems We're Solving

1. **Context management** — large codebases overwhelm agents. Loopflow structures what goes in.

2. **Prompt reuse** — prompts live in chat logs, clipboards, scattered files. Loopflow makes them first-class artifacts.

3. **Backend portability** — tools change fast. Same prompt runs on Claude, Codex, or Gemini.

4. **Craft over vibes** — vibing produces slop. Review tasks and clear workflows enforce discipline.

---

## Target User: The Concerto

Engineers and researchers at scrappy AI labs, ML startups, research-adjacent teams. High standards for craft. Think of AI coding as co-creation, not automation.

See [target-customer.md](target-customer.md) for the full definition.

---

## Strategic Positioning

**Don't compete with terminals.** Warp, Ghostty, iTerm are great. Let people use what they like.

**Don't reimplement agents.** Claude Code, Codex, Gemini CLI do the hard work. We assemble context and prompts.

**Own the prompts.** Portable, versioned, reusable. When something works, you can find it again.

---

## Build vs Leverage

| Build | Leverage |
|-------|----------|
| Task execution | Claude/Codex/Gemini (agents) |
| Context assembly | Git |
| Prompt storage | Filesystem + git |
| Concerto UI | Native macOS |

**Don't build:** Custom agents, custom hooks, proprietary formats, IDE integrations

---

## Success Metrics

**Users:** Time to first task < 5 min. Prompts they actually reuse. Design intent survives sessions.

**Loopflow:** Users create own tasks. Retention as models improve. Solving problems agents can't.

---

## Risks

| Risk | Mitigation |
|------|------------|
| Claude Code absorbs prompt features | Stay portable—cross-backend layer |
| Model improvements reduce need | Quality and reproducibility still matter |
| Too complex for casual users | Sensible defaults. `lf debug -v` just works. |

---

*January 2026*
