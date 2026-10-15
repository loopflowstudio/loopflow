# Loopflow Visual Design System

Desktop uses the semantic roles in `LoopflowPalette`. The website shares the
palette and typography, with separate marketing conventions below.

## Desktop semantics

Select a Task: neutral tint plus edge marks selection while the title stays neutral.
Tab to a control: a separate burgundy/rose outline marks keyboard focus, including
on a selected row with an error. Ordinary controls use neutral ink; only an explicitly
emphasized action uses burgundy. Repo identity may retain burgundy. Links use
secondary ink and underline on hover/focus in navigation and control chrome.

| Role | Source / behavior |
|------|-------------------|
| Structure and text | `background`, surface ladder, `text`, `textSecondary`, `textTertiary` |
| Selection | `selectionFill`, `selectionEdge`; independent of failure |
| Keyboard focus | `keyboardFocus`; visible with selection |
| Links | `linkInk`, `linkHover`; separate from status |
| Actions | `actionInk`, `actionHover`; `emphasisInk` for deliberate emphasis |
| Program activity | `working` blue, `waiting` amber, `failure` red, `success` green |

Unknown and client-liveness-only observations have no activity label or dot. Shared
Waiting remains explicitly inferred; reported Blocked is not renamed Waiting.
Program Done is not Task completion. Workflow position uses neutral marking; a
stopped edge alone is not a red error. Read failures remain inspectable.

`ThemeDefinition` owns explicit sRGB terminal colors and the semantic
`LoopflowPalette` used by app and pane chrome. Ghostty receives a normalized color
overlay after personal non-color configuration. Retained surfaces update in place.
Imports preserve terminal ANSI; they never map ANSI indices to activity. Application
surfaces derive from background polarity, with independently readable semantic ink.
Optional block fills diminish when they would hide default text.

Built-ins and imported Desktop appearances have explicit roles. `WorkPaletteTests`
resolves actual SwiftUI colors and composites selection fills over each surface: ordinary text
must reach 4.5:1, meaningful indicators 3:1. Static marketing hex examples below
are not Desktop status colors or contrast evidence.

## Marketing reference

---

## Brand Foundation

### Logo
The logo uses a four-direction gradient:
- **Wine**: `#9B1A4A` — deep, vibrant red-violet
- **Cyan**: `#0AB3CC` — cool, technical

### Design Philosophy
Loopflow's visual identity evokes **classical instruments, wine, and craftsmanship**—warm and refined, not cold tech blue. The design signals that we value craft *and* throughput, not either/or.

| Visual Choice | Brand Signal |
|---------------|--------------|
| Burgundy accent | Warmth, craft, classical instruments |
| Serif headlines | Editorial quality, intentionality |
| Cream backgrounds | Clarity, nothing to hide |
| Tight spacing (980px) | Considered, not bloated |
| Minimal navigation | Confidence—doesn't need to shout |

---

## Colors

### Primary Accent

| Token | Hex | RGB | Usage |
|-------|-----|-----|-------|
| `burgundy` | `#722F37` | 114, 47, 55 | Headings, CTAs, links, focus states |
| `burgundy-hover` | `#8B3D47` | 139, 61, 71 | Hover and active states |

The burgundy is the logo wine's "indoor voice"—same family, but appropriate for sustained UI use.

### Light Mode (Cream)

| Token | Hex | Usage |
|-------|-----|-------|
| `background` | `#FAF8F5` | Main page background |
| `surface` | `#FFFDFB` | Elevated cards, modals |
| `surface-muted` | `#F3EEE7` | Secondary surfaces, code blocks |
| `border` | `#E3DDD5` | Borders, dividers |
| `text` | `#1A1A1A` | Primary text |
| `text-secondary` | `#6B6B6B` | Secondary text, captions |

### Dark Mode (Slate)

| Token | Hex | Usage |
|-------|-----|-------|
| `background` | `#2B3036` | Main page background |
| `surface` | `#343B44` | Elevated cards, modals |
| `surface-muted` | `#3C4550` | Secondary surfaces |
| `border` | `#46505B` | Borders, dividers |
| `text` | `#F5F1EA` | Primary text |
| `text-secondary` | `#C8C1B8` | Secondary text |

### Status Colors

| Token | Hex | Usage |
|-------|-----|-------|
| `success` | `#2D6A4F` | Stable, complete, passing |
| `warning` | `#B0812A` | Early-stage, caution |
| `error` | `#B45309` | Experimental, failing |
| `info` | `#0AB3CC` | Informational (logo cyan) |
| `neutral` | `#8B8B8B` | Idle, inactive, completed |

### CSS Variables

```css
:root {
  /* Brand */
  --burgundy: #722F37;
  --burgundy-hover: #8B3D47;

  /* Backgrounds */
  --bg: #FAF8F5;
  --bg-surface: #FFFDFB;
  --bg-muted: #F3EEE7;
  --border: #E3DDD5;

  /* Text */
  --text: #1A1A1A;
  --text-secondary: #6B6B6B;

  /* Status */
  --success: #2D6A4F;
  --warning: #B0812A;
  --error: #B45309;
  --info: #0AB3CC;
  --neutral: #8B8B8B;
}

@media (prefers-color-scheme: dark) {
  :root {
    --bg: #2B3036;
    --bg-surface: #343B44;
    --bg-muted: #3C4550;
    --border: #46505B;
    --text: #F5F1EA;
    --text-secondary: #C8C1B8;
  }
}
```

### Swift Colors

```swift
extension Color {
    // Brand
    static let loopflowBurgundy = Color(hex: 0x722F37)
    static let loopflowBurgundyHover = Color(hex: 0x8B3D47)

    // Light mode
    static let loopflowCream = Color(hex: 0xFAF8F5)
    static let loopflowCreamElevated = Color(hex: 0xFFFDFB)
    static let loopflowCreamMuted = Color(hex: 0xF3EEE7)

    // Dark mode
    static let loopflowSlate = Color(hex: 0x2B3036)
    static let loopflowSlateElevated = Color(hex: 0x343B44)
    static let loopflowSlateMuted = Color(hex: 0x3C4550)

    // Status
    static let statusSuccess = Color(hex: 0x2D6A4F)
    static let statusWarning = Color(hex: 0xB0812A)
    static let statusError = Color(hex: 0xB45309)
    static let statusInfo = Color(hex: 0x0AB3CC)
    static let statusNeutral = Color(hex: 0x8B8B8B)
}
```

---

## Typography

Three pillars: classical serif for presence, warm humanist sans for readability, monospace for code.

### The System

| Role | Font | Variable | Usage |
|------|------|----------|-------|
| Serif | **Cormorant Garamond** | `--font-serif` | Headlines, taglines, hero text |
| Sans | **Lato** | `--font-sans` | Body text, buttons, navigation, UI |
| Mono | **JetBrains Mono** | `--font-mono` | Code, terminal, technical content |

### Why These Fonts

**Cormorant Garamond** — Classical Garamond inspiration with deeply calligraphic italics. The italic **f** has an elongated S-curve reminiscent of a violin f-hole—a subtle nod to the musical/orchestral metaphor.

**Lato** — Warm humanist sans that pairs naturally with Garamonds. Semi-rounded details give warmth without being soft.

**JetBrains Mono** — Developer-focused monospace with clear distinction between similar characters (0/O, 1/l/I).

### CSS

```css
:root {
  --font-serif: 'Cormorant Garamond', Georgia, serif;
  --font-sans: 'Lato', -apple-system, sans-serif;
  --font-mono: 'JetBrains Mono', monospace;
}

/* Headings default to burgundy */
h1, h2, h3 {
  color: var(--burgundy);
}
```

### Swift

```swift
enum Typography {
    static let serifFamily = "Cormorant Garamond"
    static let sansFamily = "Lato"
    static let monoFamily = "JetBrains Mono"
}
```

### Font Weights

```
Cormorant Garamond: 400, 500, 600, 700 (roman + italic)
Lato: 400, 700, 900
JetBrains Mono: 400, 500
```

### Italic Serif

Reserved for special moments:
- Taglines ("Agents that remember. Work that compounds.")
- Pull quotes
- Emphasis in headlines

**Not** the default for regular emphasis—use bold or sans-serif instead.

---

## Spacing

Based on a 4pt grid. Use semantic names, not arbitrary values.

| Token | Value | Usage |
|-------|-------|-------|
| `xxs` | 2px | Hairline gaps |
| `xs` | 4px | Tight spacing |
| `sm` | 8px | Small gaps |
| `md` | 12px | Default padding |
| `lg` | 16px | Section padding |
| `xl` | 20px | Large gaps |
| `xxl` | 24px | Section margins |
| `xxxl` | 32px | Hero spacing |

### CSS

```css
/* Use semantic spacing */
padding: 16px;        /* lg */
margin-bottom: 24px;  /* xxl */
gap: 8px;             /* sm */
```

### Swift

```swift
enum Spacing {
    static let xxs: CGFloat = 2
    static let xs: CGFloat = 4
    static let sm: CGFloat = 8
    static let md: CGFloat = 12
    static let lg: CGFloat = 16
    static let xl: CGFloat = 20
    static let xxl: CGFloat = 24
    static let xxxl: CGFloat = 32
}

// Usage
.padding(.horizontal, Spacing.lg)
.padding(.vertical, Spacing.md)
```

---

## Corner Radius

| Token | Value | Usage |
|-------|-------|-------|
| `sm` | 4px | Inline code, small badges |
| `md` | 8px | Buttons, cards, code blocks |
| `lg` | 12px | Large cards |
| `xl` | 16px | Modals, install options |
| `full` | 9999px | Pills, avatars |

### CSS

```css
border-radius: 8px;   /* md - buttons, cards */
border-radius: 16px;  /* xl - modals */
```

### Swift

```swift
enum CornerRadius {
    static let sm: CGFloat = 4
    static let md: CGFloat = 8
    static let lg: CGFloat = 12
    static let xl: CGFloat = 16
    static let full: CGFloat = 9999
}
```

---

## Hit Targets

Minimum touch/click targets for accessibility.

| Context | Size |
|---------|------|
| Desktop minimum | 24×24px |
| Comfortable | 32×32px |
| Touch/mobile | 44×44px |

### Swift

```swift
enum HitTarget {
    static let minimum: CGFloat = 24
    static let comfortable: CGFloat = 32
    static let touch: CGFloat = 44
}

// Usage
Button { } label: { Image(systemName: "trash") }
    .minHitTarget()
```

---

## Z-Index Layering

| Layer | Value | Usage |
|-------|-------|-------|
| Base | 0 | Default content |
| Dropdown | 100 | Menus, popovers |
| Modal | 200 | Dialogs, sheets |
| Toast | 300 | Notifications |
| Tooltip | 400 | Hover hints |

---

## Animation

Always respect `reduceMotion` accessibility setting.

### Durations

| Type | Duration |
|------|----------|
| Fast | 100ms | Micro-interactions |
| Standard | 200ms | Most transitions |
| Slow | 300ms | Page transitions |

### Swift

```swift
@Environment(\.accessibilityReduceMotion) private var reduceMotion

// Always use helpers that respect accessibility
withAnimation(DesignAnimation.standard(reduceMotion)) {
    isExpanded.toggle()
}

enum DesignAnimation {
    static func standard(_ reduceMotion: Bool) -> Animation?
    static func fast(_ reduceMotion: Bool) -> Animation?
    static func spring(_ reduceMotion: Bool) -> Animation?
}
```

### CSS

```css
@media (prefers-reduced-motion: reduce) {
    * { transition: none !important; }
}
```

---

## Accessibility

### Focus States

All interactive elements must have visible focus indicators:

```css
:focus {
    outline: 2px solid var(--burgundy);
    outline-offset: 2px;
}
```

### Color Contrast

All text meets WCAG AA standards:

| Combination | Ratio | Grade |
|-------------|-------|-------|
| `#1A1A1A` on `#FAF8F5` | 15.2:1 | AAA |
| `#6B6B6B` on `#FAF8F5` | 5.1:1 | AA |
| `#722F37` on `#FAF8F5` | 7.8:1 | AAA |
| `#F5F1EA` on `#2B3036` | 11.4:1 | AAA |

### Swift Accessibility

```swift
// Icon-only buttons need labels
Button { delete() } label: { Image(systemName: "trash") }
    .accessibilityLabel("Delete")
    .accessibilityHint("Removes this item permanently")
    .minHitTarget()

// Group related elements
VStack { Text(title); Text(subtitle) }
    .accessibilityElement(children: .combine)
```

---

## Verification Checklist

Before merging UI changes:

- [ ] Uses semantic spacing tokens, not arbitrary values
- [ ] Buttons have minimum hit target (24px desktop, 44px touch)
- [ ] Focus states are visible
- [ ] Respects `prefers-reduced-motion` / `reduceMotion`
- [ ] Icon-only buttons have accessibility labels
- [ ] Color contrast meets WCAG AA (4.5:1 for text)
- [ ] Desktop headings remain neutral; marketing headings may use burgundy
- [ ] Test with VoiceOver/screen reader enabled
