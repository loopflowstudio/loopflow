# Context

## Vision

Agents see the right things at the right time. The prompt pipeline is the product — what agents see determines what they can do. Today the context system works but is opaque: you can't tell what's eating tokens, and directions require flags every time.

Transparent (you see exactly what's in the prompt) and smart about defaults (good context without flags).

### Not here

- Prompt XML structure changes (the `<lf:docs>` wrapper is fine)
- New document sources (e.g., git blame, issue tracker)
- Summaries — experimental and not enabled anywhere. Separate effort once validated.

## Strategy

Make directions effortless via defaults and aliasing. Context visibility (audit breakdown in CLI, context panel in Concerto) is shipped — remaining work is direction aliases and context UI polish.

## Goals

- Directions flow from config defaults and personal aliases, not just CLI flags
- Context UI polish: per-file detail beyond top 10, budget customization display, stacked bar precision for small sources

## Risks

- **Config complexity.** More defaults means more places to look when behavior is surprising. The audit breakdown mitigates this.
- **Large document lists.** `ContextSnapshot` serializes the full document list per source. Currently unbounded on the Rust side — Concerto caps display at 10 per source. Not a concern at current scale but worth watching if sessions grow to hundreds of documents.

## Metrics

- % of sessions using default directions from config vs explicit `-d` flags (adoption rate)
- Token count per context source visible in audit header and Concerto
- Context token budget utilization: % of budget consumed by area docs (track to detect bloat)
