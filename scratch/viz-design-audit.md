# Design Audit: Concerto vs VISUAL_DESIGN.md

## Problem

VISUAL_DESIGN.md defines a complete design system. Concerto has matching tokens in code (`StatusColors.swift`, `BrandColors.swift`, `DesignSystem.swift`). But most views bypass the tokens and use SwiftUI system defaults — system colors, system fonts, literal spacing values. The tokens exist; adoption doesn't.

The original known deviation (neon green status) has already been fixed. The real problem is broader: ~30 system color instances, ~150 system font instances, ~15 hardcoded corner radii. The design system is infrastructure without tenants.

## Approach

Produce an audit report mapping every deviation to a specific fix, prioritized by visual impact per line changed. The report (`reports/viz/design-audit.md`) feeds directly into Phase 4 polish PRs.

Five categories of deviation found:

1. **System colors instead of tokens** (~30 instances, 10 files) — `.blue`, `.orange`, `.gray`, `.purple` used where `statusInfo`, `statusWarning`, etc. should be
2. **System fonts instead of Typography** (~150 instances, 20 files) — `.font(.system(size:))` instead of `Typography.body()`, `Typography.code()`, etc.
3. **Hardcoded corner radius** (~15 instances) — literal `cornerRadius: 8` instead of `CornerRadius.md`
4. **Hardcoded spacing** (~8 instances) — literal padding values instead of `Spacing` tokens
5. **Single button style** — only `DarkButtonStyle` exists; no ghost/outline/destructive variants

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Fix everything in one PR | Complete but risky, hard to review | Phase 4 explicitly calls for small, focused diffs (20-200 lines each) |
| Fix only colors | Quick win but leaves typography as biggest gap | Typography is the most visible deviation |
| Redesign the token system | Could improve API but delays all fixes | Tokens are fine — adoption is the problem |

## Key decisions

**Token adoption over token redesign.** The tokens match VISUAL_DESIGN.md. The gap is usage, not definition. "One change per PR" from `07-polish.md` — each tier becomes its own PR.

**Add `statusNeutral` token.** `.gray` appears ~15 times for idle/completed/inactive states. The design system has no neutral status color. Adding one eliminates the most common system color and makes the intent explicit. Per the wave's README: "Audit current design against VISUAL_DESIGN.md" — the audit should also surface gaps in the spec, not just the code.

**Font bundling is a prerequisite.** Typography tokens call custom fonts (Cormorant Garamond, Lato, JetBrains Mono). If these aren't in the app bundle, the calls silently fall back to SF Pro. Must verify and fix before the typography PR has any visible effect.

**Button hierarchy from visual research.** `reports/viz/visual-research.md` finding #3: "Reduce action button visual weight. Ghost buttons for secondary actions, filled for primary." Three styles needed: primary (existing), ghost (new), destructive (new).

## Scope

- In scope: audit report with every deviation catalogued, prioritized fix list, recommended PR sequence
- Out of scope: implementing the fixes (that's Phase 4), changing VISUAL_DESIGN.md beyond adding missing tokens

## Done when

`reports/viz/design-audit.md` contains a complete deviation map with prioritized fixes and a PR sequence for Phase 4.
