# LLM Design Guidelines Integration

Incorporate visual design guidelines from Vercel and UI Skills into the Designer goal and Maestro DESIGN.md.

## What to Build

Add actionable implementation guidelines to two files:
1. `.lf/goals/designer.md` — expand from architecture-only to include visual/UI design rules
2. `Maestro/DESIGN.md` — add an implementation reference section with specific rules

## Data Structures

N/A — these are markdown documentation files.

## Key Changes

### `.lf/goals/designer.md`

Current state: focused on software architecture ("abstractions and boundaries"). Needs to cover visual design when the task involves UI.

Add a new section with the most critical visual design rules:

```markdown
## Visual Design Rules

When designing UI components:

### Typography
- Use `text-balance` for headings, `text-pretty` for body text
- Use `tabular-nums` for numeric data
- Curly quotes ("") not straight quotes ("")
- Ellipsis character (…) not three periods (...)
- Non-breaking spaces for units: `10&nbsp;MB`

### Layout
- Optical alignment over geometric when it looks better (±1px)
- Child border-radius must not exceed parent radius
- Use `h-dvh` not `h-screen` (mobile viewport)
- Respect safe areas with `env()` variables

### Animation
- Only animate `transform` and `opacity`
- Never animate width, height, margins, padding
- Respect `prefers-reduced-motion`
- Interaction feedback under 200ms

### Accessibility
- All flows keyboard-operable per WAI-ARIA patterns
- Visible focus rings via `:focus-visible`
- Hit targets ≥24px (≥44px on mobile)
- Icon-only buttons require `aria-label`
- Every form control has a `<label>`

### Color
- One accent color per view
- Prefer APCA over WCAG 2 for contrast
- Set `color-scheme: dark` for dark themes
```

### `Maestro/DESIGN.md`

Current state: excellent high-level principles but lacks specific implementation rules. Add a new section after "Visual Design Direction" with the Vercel/UI Skills guidelines.

New section title: **Implementation Reference**

Organize by category:
- Typography & Text
- Layout & Spacing
- Shadows & Depth
- Color & Contrast
- Keyboard & Focus
- Touch & Mobile
- Forms
- Animation
- Performance
- State Management
- Accessibility

## Constraints

- Don't duplicate what's already in DESIGN.md — the new section supplements existing principles
- Keep the Designer goal concise — it's a goal file, not a reference manual
- Attribution: note the sources (Vercel Design Guidelines, UI Skills)

## UI Changes

N/A — documentation only.

## Done When

```bash
grep -q "tabular-nums" .lf/goals/designer.md && \
grep -q "Implementation Reference" Maestro/DESIGN.md && \
echo "Both files updated"
```

Expected output: `Both files updated`
