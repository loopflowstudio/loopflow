# Maestro Design System + Accessibility

Formalize spacing, typography, and accessibility patterns based on UI Skills and Vercel design guidelines.

## What to build

A `DesignSystem.swift` file with spacing/typography scales, plus accessibility labels and motion preference support across all Maestro views.

## Current State

**Spacing values found**: 0, 1, 3, 4, 6, 8, 10, 12, 14, 16, 20, 24 (ad-hoc)

**Typography**: Mix of semantic (`.caption`, `.subheadline`) and explicit (`.system(size: 14)`)

**Accessibility**: Zero `accessibilityLabel`, `accessibilityIdentifier`, or `accessibilityHint` in Views

**Animations**: 14 `withAnimation` calls, none check `reduceMotion`

**Hit targets**: Some interactive elements are 6x6 or 8x8 (below 24px minimum)

## Data structures

```swift
// Maestro/Maestro/DesignSystem.swift

import SwiftUI

// MARK: - Spacing Scale (4pt base)

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

// MARK: - Hit Targets (Vercel: 24px desktop minimum)

enum HitTarget {
    static let minimum: CGFloat = 24
    static let comfortable: CGFloat = 32
    static let touch: CGFloat = 44  // iOS guideline
}

// MARK: - Z-Index Scale (UI Skills: fixed scale, no arbitrary values)

enum ZIndex {
    static let base: Double = 0
    static let dropdown: Double = 100
    static let modal: Double = 200
    static let toast: Double = 300
    static let tooltip: Double = 400
}

// MARK: - Corner Radius

enum CornerRadius {
    static let sm: CGFloat = 4
    static let md: CGFloat = 8
    static let lg: CGFloat = 12
    static let xl: CGFloat = 16
    static let full: CGFloat = 9999
}

// MARK: - Typography
// Adapted from loopflowstudio/website/TYPOGRAPHY.md
// Serif: Cormorant Garamond - headlines, hero
// Sans: Lato - body, UI
// Mono: JetBrains Mono - code, terminal

enum Typography {
    // Font families (must be installed or bundled)
    static let serifFamily = "Cormorant Garamond"
    static let sansFamily = "Lato"
    static let monoFamily = "JetBrains Mono"

    // Semantic styles
    static func heroTitle(_ size: CGFloat = 32) -> Font {
        .custom(serifFamily, size: size).weight(.semibold)
    }

    static func sectionTitle(_ size: CGFloat = 20) -> Font {
        .custom(serifFamily, size: size).weight(.medium)
    }

    static func body(_ size: CGFloat = 14) -> Font {
        .custom(sansFamily, size: size)
    }

    static func bodyBold(_ size: CGFloat = 14) -> Font {
        .custom(sansFamily, size: size).weight(.bold)
    }

    static func caption(_ size: CGFloat = 12) -> Font {
        .custom(sansFamily, size: size)
    }

    static func code(_ size: CGFloat = 13) -> Font {
        .custom(monoFamily, size: size)
    }

    static func codeSmall(_ size: CGFloat = 11) -> Font {
        .custom(monoFamily, size: size)
    }

    // Fallback to system fonts if custom fonts unavailable
    static func systemBody() -> Font { .body }
    static func systemCaption() -> Font { .caption }
    static func systemMono() -> Font { .system(.body, design: .monospaced) }
}

// MARK: - Animation (respects reduce motion)

struct DesignAnimation {
    @Environment(\.accessibilityReduceMotion) static var reduceMotion

    static func standard(_ reduceMotion: Bool) -> Animation? {
        reduceMotion ? nil : .easeInOut(duration: 0.2)
    }

    static func fast(_ reduceMotion: Bool) -> Animation? {
        reduceMotion ? nil : .easeOut(duration: 0.1)
    }

    static func spring(_ reduceMotion: Bool) -> Animation? {
        reduceMotion ? nil : .spring(response: 0.3, dampingFraction: 0.7)
    }
}
```

## Key functions

```swift
// View modifier for accessible buttons
extension View {
    func accessibleButton(_ label: String, hint: String? = nil) -> some View {
        self
            .accessibilityLabel(label)
            .accessibilityHint(hint ?? "")
            .accessibilityAddTraits(.isButton)
    }

    func accessibleToggle(_ label: String, isOn: Bool) -> some View {
        self
            .accessibilityLabel(label)
            .accessibilityValue(isOn ? "On" : "Off")
            .accessibilityAddTraits(.isButton)
    }
}

// Animation wrapper that respects motion preferences
extension View {
    func animateIf(_ condition: Bool, _ animation: Animation?) -> some View {
        self.animation(condition ? nil : animation, value: condition)
    }
}
```

## Font Installation

The typography system uses custom fonts. Two options:

**Option A: System-installed fonts (simpler)**
User installs Cormorant Garamond, Lato, JetBrains Mono via Google Fonts. App falls back to system fonts if unavailable.

**Option B: Bundled fonts (reliable)**
Add font files to `Maestro/Maestro/Resources/Fonts/` and register in Info.plist:
```xml
<key>ATSApplicationFontsPath</key>
<string>Fonts</string>
```

Start with Option A (fallback to system fonts). Bundle fonts later if needed.

## Typography Usage

| Context | Font | Example |
|---------|------|---------|
| Page/section titles | Serif (Cormorant) | "Worktrees", "Output" |
| Body text, labels | Sans (Lato) | Button labels, descriptions |
| Code, commits, tokens | Mono (JetBrains) | SHA hashes, file paths |

Use serif sparingly—headlines and section titles only. Sans is the workhorse.

## Changes by file

### DesignSystem.swift (new)
- Spacing enum
- HitTarget enum
- ZIndex enum
- CornerRadius enum
- Typography enum with font helpers
- Animation helpers
- Accessibility view modifiers

### BrandColors.swift
Add to LoopflowPalette:
```swift
// Status colors (already used inline, formalize them)
static let success = Color.green
static let error = Color.red
static let warning = Color.orange
static let info = Color.blue
```

### Views requiring accessibility labels

| View | Element | Label |
|------|---------|-------|
| WorktreeSidebar | Worktree row | "Worktree: {branch}" |
| WorktreeSidebar | PR status badge | "Pull request {status}" |
| WorktreeSidebar | New worktree button | "Create new worktree" |
| WorktreeDetailPanel | Action buttons | "Open in terminal", "Open in IDE", etc. |
| WorktreeDetailPanel | Land button | "Land pull request" |
| PromptLauncher | Task selector | "Select task" |
| PromptLauncher | Run button | "Run {task}" |
| PromptLauncher | Context toggles | "Include {context type}" |
| ContextChip | Toggle | "{name}, {on/off}" |
| OutputPanel | Session selector | "Select session" |
| ResultsPanel | Expand/collapse | "Expand results" |

### Views requiring motion preference checks

| View | Animation | Current |
|------|-----------|---------|
| WorktreeDetailPanel:507 | Section expand | `withAnimation(.easeInOut(duration: 0.2))` |
| ResultsPanel:118 | Panel expand | `withAnimation(.easeInOut(duration: 0.2))` |
| ResultsPanel:233 | Context toggle | `withAnimation(.easeInOut(duration: 0.15))` |
| OutputPanel:101 | Panel expand | `withAnimation(.easeInOut(duration: 0.2))` |
| PromptLauncher:795 | Dropdown | `withAnimation(.easeInOut(duration: 0.2))` |
| PromptLauncher:915 | Context section | `withAnimation(.easeInOut(duration: 0.15))` |

Replace with:
```swift
@Environment(\.accessibilityReduceMotion) var reduceMotion

withAnimation(reduceMotion ? nil : .easeInOut(duration: 0.2)) {
    // ...
}
```

### Views requiring hit target fixes

| View | Element | Current | Fix |
|------|---------|---------|-----|
| WorktreeSidebar:170 | Status dot | 6x6 | Decorative only - OK |
| OutputPanel:62 | Clickable dot | 8x8 | Wrap in 24x24 tap area |
| ResultsPanel:514 | Expand indicator | 8x8 | Wrap in 24x24 tap area |

## Spacing migration (gradual)

Replace inline values with Spacing constants. Priority order:
1. `.padding()` calls in reusable components (ContextChip, FileChip)
2. Section headers/dividers
3. Card/panel padding
4. Row spacing

Example migration:
```swift
// Before
.padding(.horizontal, 16)
.padding(.vertical, 12)

// After
.padding(.horizontal, Spacing.lg)
.padding(.vertical, Spacing.md)
```

## UI changes

None visible. This is infrastructure-only - design system formalization and accessibility improvements. No Maestro UI feature changes.

## Constraints

- **Don't change visual appearance** - spacing values should map to current usage
- **Gradual migration** - don't rewrite all views at once; start with DesignSystem.swift and new code
- **macOS 15+ only** - can use modern SwiftUI APIs

## Done when

1. `DesignSystem.swift` exists with Spacing, HitTarget, ZIndex, CornerRadius, Typography enums
2. All 14 `withAnimation` calls check `reduceMotion`
3. Interactive elements in WorktreeSidebar, PromptLauncher, and WorktreeDetailPanel have `accessibilityLabel`
4. VoiceOver can navigate the main flow: select worktree → select task → run
5. At least one view uses Typography helpers (demonstration of pattern)

Verify:
```bash
# Build succeeds
cd Maestro && xcodebuild -scheme Maestro -configuration Debug build

# DesignSystem.swift exists with all enums
grep -E "enum (Spacing|HitTarget|ZIndex|CornerRadius|Typography)" Maestro/Maestro/DesignSystem.swift | wc -l
# Should be 5

# Check accessibility labels exist
grep -r "accessibilityLabel" Maestro/Maestro/Views/ | wc -l
# Should be > 20

# Check reduceMotion is respected
grep -r "reduceMotion" Maestro/Maestro/Views/ | wc -l
# Should be >= 14
```
