# Loopflow Orchestra

## The Metaphor

**Maestro** — You're the conductor. Agents are your musicians. You direct them to create software.

**Orchestra** — Engineers are the musicians. Each plays their instrument (Maestro + agents). Together you create one piece of software.

| | Maestro | Orchestra |
|---|---|---|
| Music | The software | The software |
| Sheet music | Prompts | Shared prompts |
| Musicians | Agents | Engineers |
| Conductor | You | — |
| Instrument | — | Maestro + agents |

In Orchestra, coordination is between humans—the engineers playing together. Each musician brings their own instrument (their Maestro, their agents). The challenge isn't directing agents; it's playing in harmony with other engineers.

The metaphor is a values reminder: play to serve the music, not yourself. The software is what matters.

## What Orchestra Adds

### Shared sheet music

Prompts live in the repo. Everyone works from the same playbook. When someone improves a prompt, the whole team benefits.

```
.claude/commands/
  review.md      # team's review standards
  implement.md   # how we build features
  debug.md       # how we fix bugs
```

This already exists in Loopflow v1. Orchestra makes it a team workflow, not just a personal one.

### Sections

Different engineers work on different parts. Worktrees keep them isolated. The merge queue coordinates when parts come together.

```
alice/auth-feature     # Alice conducting auth work
bob/api-refactor       # Bob conducting API work
carol/perf-fixes       # Carol conducting performance
```

### Quality gates

CI and merge queues ensure the parts work together before shipping. Standard team infrastructure, not unique to Orchestra—but required for teams to work safely.

```
PR created → CI runs
Merge queue → CI runs again after rebase
Main → Ship
```

### Visibility

Who's working on what? What's the status? Orchestra shows the whole ensemble:

- Active worktrees across the team
- Running agents and their progress
- Queue of work waiting to land

## What This Means for the MVP

CI + merge queue is table stakes for teams. Not Orchestra-specific, but required before Orchestra makes sense.

The MVP:
1. CI runs tests on PRs
2. Merge queue gates main
3. `lfops land` submits to queue

## Future: Hosted Orchestra

Today, each engineer runs agents locally. Future Orchestra could provide:

- **Shared compute** — agents run on Loopflow servers, not your laptop
- **Team dashboard** — see all active work across the team
- **Async handoff** — start work, hand off to teammate, they continue

But that's later. The foundation is shared prompts + merge queue coordination.

## Open Questions

1. **Is "Orchestra" the product name or just internal framing?**

2. **What visibility do teams need?** Worktree status? Agent output? Diff previews?

3. **How do teams share prompts across repos?** Monorepo? Shared package? Copy-paste?
