---
pm:
  provider: linear
  linear_project: '8c4ba3f9-cf23-4136-87ed-37847aa7dc82'
---

## Objective

Collapse the three binaries — `lf`, `lfd`, `lfq` — toward **one workhorse plus one
thin server**. `lf` already does the real behavior (run steps/flows, `lf wave`
runs a wave) and needs shell/binary/ssh access; that is the only access
model. Everything lfd and lfq do that is *exec-behavior* moves into `lf` (`lf d`
for store reads/writes + tmux/docker launch, `lf q` for queue/worker). `lfd`
shrinks to a **guarded subscription server** whose sole justification is push —
the live wave-status and terminal-output streams Concerto needs, which a transient
`lf` invocation cannot be. It execs `lf`; it does not reimplement behavior. This
is an architecture wave: each note should leave the system smaller and more true. Not a
rewrite — a collapse of concepts, net-negative code each pass.

## Measures

- **Key Results**: binaries move from 3 to 2 (`lf`, `lfd serve`); `lfq` is deleted.
- **Key Results**: lfd LOC falls; only the subscription hub and guarded external interface survive. Target: net-negative on every landed item.
- **Quality**: behavior reimplemented across binaries stays at 0 — `lfd serve` execs `lf`, never duplicates it.
- **Key Results**: `lf`-launched sessions visible in live status: 0% -> 100% via the registry.
- **Done means**: a landed PR of real product code, roadmap item closed and PR-linked.

## Process

Read the live roadmap and choose the smallest collapse that leaves the system
more true. Use direct implementation for mechanical deletions; write a scratch
design and review pass for cross-boundary moves. Routing is prose judgment, not
frontmatter.
