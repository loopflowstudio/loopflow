# Loopflow Color System

Unified color palette for loopflow (Swift app) and loopflowstudio (website).

---

## Brand Foundation

### Logo Gradient
The logo uses a four-direction gradient anchored by two colors:
- **Wine**: `#9B1A4A` — deep, vibrant red-violet, the "hot" end
- **Cyan**: `#0AB3CC` — cool, technical, the "cool" end

### Primary Accent: Burgundy
The primary UI accent is **not** the logo wine—it's a deeper, more refined burgundy that evokes red wine, jazz guitars, and classical instruments.

```
Burgundy:       #722F37  rgb(114, 47, 55)
Burgundy Hover: #8B3D47  rgb(139, 61, 71)
```

This burgundy sits between the logo's wine and a true brown—warm enough to feel inviting, dark enough to feel serious.

---

## The Palette

### Light Mode (Cream)

| Token | Hex | RGB | Usage |
|-------|-----|-----|-------|
| `background` | `#FAF8F5` | 250, 248, 245 | Main page background |
| `surface` | `#FFFDFB` | 255, 253, 251 | Elevated cards, modals |
| `surface-muted` | `#F3EEE7` | 243, 238, 231 | Secondary surfaces, code blocks |
| `border` | `#E3DDD5` | 227, 221, 213 | Borders, dividers |
| `text` | `#1A1A1A` | 26, 26, 26 | Primary text |
| `text-secondary` | `#6B6B6B` | 107, 107, 107 | Secondary text, captions |
| `accent` | `#722F37` | 114, 47, 55 | CTAs, links, highlights |
| `accent-hover` | `#8B3D47` | 139, 61, 71 | Hover states |

### Dark Mode (Slate)

| Token | Hex | RGB | Usage |
|-------|-----|-----|-------|
| `background` | `#2B3036` | 43, 48, 54 | Main page background |
| `surface` | `#343B44` | 52, 59, 68 | Elevated cards, modals |
| `surface-muted` | `#3C4550` | 60, 69, 80 | Secondary surfaces |
| `border` | `#46505B` | 70, 80, 91 | Borders, dividers |
| `text` | `#F5F1EA` | 245, 241, 234 | Primary text |
| `text-secondary` | `#C8C1B8` | 200, 193, 184 | Secondary text |
| `accent` | `#722F37` | 114, 47, 55 | CTAs, links |
| `accent-hover` | `#8B3D47` | 139, 61, 71 | Hover states |

### Status Colors

| Token | Hex | Usage |
|-------|-----|-------|
| `success` | `#2D6A4F` | Stable, complete, passing |
| `warning` | `#B0812A` | Early-stage, caution |
| `error` | `#B45309` | Experimental, failing |
| `info` | `#0AB3CC` | Informational (logo cyan) |

---

## Color Relationships

```
Logo Wine (#9B1A4A)
        ↓
        ↓  Darkened for UI use
        ↓
Primary Burgundy (#722F37) ← UI accent color
        ↓
        ↓  Lightened for hover
        ↓
Burgundy Hover (#8B3D47)
```

The burgundy is derived from the logo wine by reducing lightness for a more sophisticated, grounded feel appropriate for sustained interface use.

---

## CSS Variables

```css
:root {
  /* Brand */
  --burgundy: #722F37;
  --burgundy-hover: #8B3D47;

  /* Light mode backgrounds */
  --bg: #FAF8F5;
  --bg-surface: #FFFDFB;
  --bg-muted: #F3EEE7;
  --border: #E3DDD5;

  /* Light mode text */
  --text: #1A1A1A;
  --text-secondary: #6B6B6B;

  /* Status */
  --success: #2D6A4F;
  --warning: #B0812A;
  --error: #B45309;
  --info: #0AB3CC;
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

---

## Swift Colors

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

    // Text
    static let loopflowText = Color(hex: 0x1A1A1A)
    static let loopflowTextSecondary = Color(hex: 0x6B6B6B)
    static let loopflowTextLight = Color(hex: 0xF5F1EA)
    static let loopflowTextSecondaryLight = Color(hex: 0xC8C1B8)

    // Info (logo cyan)
    static let loopflowInfo = Color(hex: 0x0AB3CC)
}
```

---

## Why Not Use Logo Colors Directly?

The logo's `#9B1A4A` wine is designed to **pop**—it's a hero color for brand recognition. Using it as a UI accent would:

1. **Compete with the logo** instead of supporting it
2. **Feel too hot** for sustained interface use
3. **Reduce contrast** against light backgrounds (WCAG concerns)

The burgundy `#722F37` is the logo wine's "indoor voice"—the same family, but appropriate for buttons, links, and focus states.

---

## Contrast Ratios

All text colors meet WCAG AA standards:

| Combination | Ratio | Grade |
|-------------|-------|-------|
| `#1A1A1A` on `#FAF8F5` | 15.2:1 | AAA |
| `#6B6B6B` on `#FAF8F5` | 5.1:1 | AA |
| `#722F37` on `#FAF8F5` | 7.8:1 | AAA |
| `#F5F1EA` on `#2B3036` | 11.4:1 | AAA |
| `#C8C1B8` on `#2B3036` | 7.2:1 | AAA |

---

## Typography Pairing

The color system is designed to work with:

| Role | Font | Weight |
|------|------|--------|
| Serif (display) | Cormorant Garamond | 400–700 |
| Sans (body) | Lato | 400, 700, 900 |
| Mono (code) | JetBrains Mono | 400, 500 |

The warm cream backgrounds and burgundy accents complement the classical feel of Cormorant Garamond's serifs.
