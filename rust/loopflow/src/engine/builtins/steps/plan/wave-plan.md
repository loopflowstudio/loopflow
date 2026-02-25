---
requires: scratch/ analysis
produces: scratch/wave-proposal.md
---
Synthesize analysis into a wave plan — a sequenced plan for building something.

## Scope

The included context defines your area. Propose items that belong to this area. When reconciled via `update-wave`, items should live in `wave/<wave>/`.

## Workflow

1. Read analysis in `scratch/` (research, simplification opportunities, polish priorities, etc.)
2. Read `wave/` to understand existing waves and project direction
3. Identify what you're building and why it matters — the core vision
4. Name the invariants, design decisions, and differentiators — what makes this system *this system* and not something generic
5. Identify the highest-leverage work that emerges from the analysis
6. Sequence it for learning — what do we need to build first to learn the most?
7. Write `scratch/wave-proposal.md`

## Vision first

Before sequencing work, capture what you're building clearly enough that someone could explain it in a conversation:

**What is this?** One paragraph. What it does, who it's for, why it exists.

**Core components.** The 3-5 pieces that make up the system. Name them. Say what each does and why it's separate from the others.

**Invariants.** Rules that must always hold. These survive sequencing changes — even if you reorder every phase, the invariants stay. State them precisely.

**Differentiators.** What makes this different from the obvious/naive approach? What design decisions define its character? Why those decisions, not the alternatives?

This section is the anchor. When phases shift and plans change, this stays. If you can't write this clearly, you don't understand the system well enough to sequence work on it.

## Sequencing principles

**Frontload the risk.** Start with the thing you need to try to see if it works. Don't pre-build infrastructure, protocols, or abstractions before you've proven the core idea. If the whole wave depends on "can we talk to X?", make that Phase 1 — not Phase 3 after you've built storage, types, and routes for a protocol you haven't validated yet.

**Build outward.** Start with the smallest thing that works end-to-end. Get concrete results, then expand. Don't build the foundation for a system you haven't proven yet.

**Sequence by learning, not dependencies.** What are you most uncertain about? Build that first. A dependency graph tells you what *could* go first. Learning tells you what *should* go first.

**Defer abstractions.** Traits, interfaces, and generic layers emerge from working code. Don't design them in advance. Build the concrete thing, then extract the pattern. Build just enough plumbing to support the first real use case — storage, API, and types emerge from making it work, not from upfront design.

**Encode uncertainty.** Mark what you're unsure about. Open questions aren't gaps in the plan — they're the most important part. Each phase should state what you expect to learn.

## Output format

Write `scratch/wave-proposal.md`:

```markdown
---
status: proposed
---

# Title

One paragraph describing what and why.

## Core

### Components

The pieces that make up the system. What each does, why it's separate.

### Invariants

Rules that always hold, regardless of implementation order.

### Design decisions

Choices that define the system's character. The "why" behind each.

## Phases

### Phase 1 — <name>

What to build. Concrete enough to start.

**What we'll learn:** What building this teaches us. What assumptions it validates.

**Open questions:** What we don't know yet. What might change.

**Checkpoint:** How we know this phase is done. What to reassess before continuing.

**Try it:** How the human should test this — both "does it work?" and "do I like this?"
- Verification: concrete command or observable outcome that proves correctness
- Feel: how to use the thing and form an opinion. What to pay attention to. What questions to ask yourself while using it.

### Phase 2 — <name>

...

## What might change

Explicit acknowledgment of where the plan is likely wrong. What would cause phases to be reordered, merged, or dropped.
```

## Guidelines

- Capture the vision and invariants before touching sequencing — they're the anchor
- Focus on substantial work, not small fixes
- Be honest about scope — what's in, what's out
- Fewer phases with learning checkpoints beat many small items in a dependency graph
- If the analysis doesn't clearly point to a proposal, write `scratch/wave-proposal.md` explaining why and what's missing
- A wave plan is a hypothesis about sequencing, not a contract. It should expect to be revised. But the vision and invariants at the top should be stable.
