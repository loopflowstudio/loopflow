---
voice: artist
---
> **Screenshots**: If running standalone, capture with Cmd+Shift+S first.
> In the `ux` pipeline, uses screenshots from the ux-research step.
>
> **Build from this branch**: Run `cd Maestro && ./dev run` to build and launch
> Maestro from the current branch. Don't use the installed app—it won't have your changes.

Compare Maestro against best-in-class tools and identify UX gaps.

Read `Maestro/DESIGN.md` first—it contains comprehensive research on the tools and thinkers we're learning from.

## Inspiration Sources

Study these products for patterns Maestro should adopt (detailed analysis in designprinciples.md):

### Figma
- Performance obsession, multiplayer presence, professional respect
- Remove friction before adding features (the "Blockers" team)
- Simplicity requires active defense

### Cursor
- Three-tier model: Tab (local), Cmd+K (scoped), Agent (autonomous)
- Show plans before execution
- Context is automatic, override is surgical (@-mentions)

### Notion
- Minimalism, slash commands, block-based composition
- Progressive disclosure—simple at surface, infinite depth
- Effortless hierarchy

### Linear
- Sub-100ms response for all interactions
- Keyboard-first with comprehensive shortcuts
- Opinionated defaults over configuration

### Stripe
- Three-column layout: navigation, content, code
- Documentation as product
- Progressive disclosure optimized for "happy path"

## Design Principles to Apply

See `Maestro/DESIGN.md` for full research. Key principles:

1. **Immediate Connection** (Bret Victor) — real-time feedback, no delays
2. **Progressive Disclosure** (Notion, Stripe) — simple surface, infinite depth
3. **Speed as Feature** (Linear, Figma) — sub-100ms, 60fps
4. **Keyboard-First** (Linear) — Cmd+K command palette
5. **Opinionated Defaults** (Linear, fast.ai) — design for someone, not everyone
6. **Graduated Autonomy** (Cursor) — match UI to scope of change
7. **Transparency Over Automation** (Cursor) — show plans before execution
8. **Design Should Disappear** (Jony Ive) — minimize chrome
9. **Remove Barriers** (fast.ai, Paper) — accessibility without patronizing
10. **Affordances Over Status** (Norman) — give users actions, not just information
11. **Craft Signals Care** (Collison, Ive) — beauty implies care

## Visual Audit

Before analyzing gaps, audit the screenshots for craft issues:

- Alignment and spacing inconsistencies
- Typography hierarchy problems
- Color contrast and accessibility
- Visual clutter or unclear affordances
- macOS platform conventions (HIG compliance)

These fall under principle #10: "Craft Signals Care."

## Gap Analysis

For each area of Maestro, identify gaps:

1. **Welcome/Setup**: vs Figma onboarding, Notion templates
2. **Prompt Input**: vs Cursor chat, Notion slash commands
3. **Context Controls**: vs Cursor's @ mentions, Figma's component panel
4. **Worktree Sidebar**: vs Notion's page tree, Figma's layers
5. **Running State**: vs Cursor's streaming, Figma's presence indicators
6. **Errors/Empty States**: vs Notion's empty pages, Figma's placeholders
   - Apply principle #10: Every error/empty state should offer an action button

## Output

Write analysis to `.design/ux-gaps.md`:

```markdown
# UX Gap Analysis

## Visual Issues
- [ ] Issue and location (from visual audit)
- [ ] ...

## Welcome/Setup
**Current**: Description of what Maestro does
**Inspiration**: What Figma/Cursor/Notion do better
**Gap**: Specific missing capability
**Pattern to adopt**: Concrete suggestion

## Prompt Input
...

## Summary: Priority Gaps
1. [Gap] - Impact: High/Medium/Low
2. ...

## Patterns to Steal
1. [Pattern from X] - Apply to [area]
2. ...
```

Do not write code. Analysis only.
