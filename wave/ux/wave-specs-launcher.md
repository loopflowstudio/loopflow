---
status: proposed
---

# Wave Specs & Launcher

Each wave item declares its execution config in frontmatter. Concerto reads them and offers batch launch.

## What to build

Wave items in `wave/` get frontmatter that describes how to run them. Concerto reads these specs and lets you pick subsets to launch. Recovery becomes: open Concerto, see what was planned vs running, reconcile.

## Wave spec format

```yaml
---
status: proposed
flow: ship
direction: [product-engineer]
area: [rust/loopflow/src/harness/]
owner: alice
---
# 01: Foundation Contract
```

Owner lives in the wave spec — the repo is the coordination artifact. Hosted loopflow reads it rather than maintaining a separate assignment system.

## Subsets / sprint files

Not all items should launch at once. Express subsets:

```yaml
# .lf/sprint.yaml
name: rust-foundation
items:
  - wave/rust/03-service.md
  - wave/harness/01-foundation-contract.md
```

Or folder-based: "launch everything `proposed` in `wave/rust/`".

## Approach

### Prompt changes

Prompts that generate wave items produce specs as part of their output:

- `lf wave-plan` — each item gets execution config in frontmatter
- `lf add-to-wave` — includes wave spec when promoting from scratch/
- `lf iterate` — preserves/updates wave specs

### Rust: `GET /waves/launchable`

Parse wave item files, extract frontmatter, cross-reference with active waves. Returns items that have specs but no active wave.

### Concerto UI

Accessible via dedicated view, command palette, or sidebar section for loopflow-configured repos.

- Wave items with specs
- Checkboxes for batch selection
- Status: planned / running / done
- "Launch selected" button

## Done when

- Wave item prompts produce execution specs in frontmatter
- `GET /waves/launchable` returns unstarted items
- Concerto shows wave items, supports batch launch
- Running waves are cross-referenced with wave items
