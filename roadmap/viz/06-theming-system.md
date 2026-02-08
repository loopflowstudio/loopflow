---
status: todo
phase: 3
---

# Theming system for fast iteration

No way to quickly try different visual approaches and get feedback. Changing colors or spacing requires rebuilding and comparing manually.

## Build

TBD — needs research from Phase 2 to inform approach. Options:

- SwiftUI environment-based theme switching (`.environment(\.theme, .burgundy)`)
- Preview variants (multiple SwiftUI previews with different themes)
- Simple token swapping ("change these 5 colors and rebuild")

Key requirement: a human can see changes fast, give feedback, iterate. The goal is rapid visual feedback, not a general-purpose theming framework.

## Done when

Visual changes can be previewed quickly without a full build cycle. A designer or developer can evaluate a color/spacing change in under a minute.
