# Context

## Vision

Understand and improve how loopflow builds context for agents. The prompt pipeline is the product — what agents see determines what they can do. Today the context system works but is opaque: you can't tell what's eating tokens, READMEs below the repo root are invisible without explicit `-a`, and directions require flags every time.

The context system should be transparent (you see exactly what's in the prompt) and smart about defaults (good context without flags).

### Not here

- Prompt XML structure changes (the `<lf:docs>` wrapper is fine)
- New document sources (e.g., git blame, issue tracker)
- Summaries — the summary system exists but is experimental and explicitly not enabled anywhere. Hardening summaries is a separate effort once we've validated they're useful.

## Strategy

Start with visibility (audit breakdown) so we can see the impact of everything else. Then fix the doc inclusion policy so agents get the right READMEs via area. Then make directions effortless via defaults and aliasing. Then surface context visibility in Concerto so it's not CLI-only.

## Goals

- See exactly what's in the prompt: scratch, wave, repo docs, area docs each get their own audit line — in both CLI and Concerto
- Agents see subdirectory READMEs when working in an area (ancestors + descendants, not siblings/uncles)
- Directions flow from config defaults and personal aliases, not just CLI flags
- This repo's config is a model for how context should be configured
- Concerto shows context breakdown visually — what's in the prompt, how tokens are spent

## Risks

- **Token bloat from recursive READMEs.** Walking descendants of an area could blow the budget in large repos. Need budget-aware gathering — drop lowest-priority docs first.
- **Config complexity.** More defaults means more places to look when behavior is surprising. The audit breakdown partially mitigates this — you can always see what's in the prompt.
- **Concerto data path.** `ContextBreakdown` lives in Rust; Concerto is SwiftUI. Need serialization and an HTTP API endpoint. Keep it async — don't block session start.

## Metrics

- Audit header shows separate token counts for scratch/, wave/, repo docs
- `lf implement -a rust/` includes descendant READMEs from `rust/**/*.md`
- `lf implement` in this repo picks up default directions from config without `-d`
- `lfq direction create` works for personal direction aliases
- Concerto session view shows per-source token breakdown
