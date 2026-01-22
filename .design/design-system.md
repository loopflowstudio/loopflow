# Design System

Unified color palette for Loopflow (Swift app) and loopflowstudio (website).

## What's Done

- **COLORS.md**: Canonical color spec with hex values, RGB, and usage guidelines
- **BrandColors.swift**: Swift implementation matching COLORS.md spec
- **globals.css**: CSS variables for web, synced with Swift

## Implementation

All three files share the same color values:

| Token | Hex |
|-------|-----|
| Burgundy | #722F37 |
| Burgundy Hover | #8B3D47 |
| Light Background | #FAF8F5 |
| Dark Background | #2B3036 |
| Info (Cyan) | #0AB3CC |

The Swift file includes:
- `loopflowInfo` in the main Color extension (matching COLORS.md spec)
- `statusInfo` as an alias to `loopflowInfo` for convenience
- `LoopflowPalette` struct for theme-aware color selection

## Verification

- Python tests: 499 passed
- Swift tests: 23 passed
- Build: successful
