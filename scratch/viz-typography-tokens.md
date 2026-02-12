---
status: todo
phase: 4
tier: 2
---

# Typography Tokens

Bundle custom fonts and replace ~150 system font calls with `Typography` tokens.

## Why this is next

Tier 1 (status color tokens) shipped in Phase 3. Typography is the next highest-leverage change — the design system specifies Cormorant Garamond for headings, Lato for body, JetBrains Mono for code, but the app renders entirely in SF Pro. Switching to custom fonts is the single biggest visual transformation available.

## Two parts

### Part 1: Bundle fonts (~5 lines)

Add Cormorant Garamond, Lato, and JetBrains Mono as app resources. Without this, `Typography` calls silently fall back to system fonts.

Verify: check if fonts are already in the Xcode project / SPM package resources. If not, add them.

### Part 2: Replace system fonts (~150 lines)

Mechanical replacement across all views:

| Current | Replacement |
|---------|-------------|
| `.font(.system(size: 13, design: .monospaced))` | `Typography.code()` |
| `.font(.system(size: 11, design: .monospaced))` | `Typography.codeSmall()` |
| `.font(.caption)` / `.font(.caption2)` | `Typography.caption()` |
| `.font(.headline)` / `.font(.title2)` | `Typography.sectionTitle()` |
| `.font(.subheadline)` / `.font(.body)` | `Typography.body()` |
| `.font(.system(size: 32))` (hero) | `Typography.heroTitle()` |

### Worst offenders by count

| File | System font instances |
|------|---------------------|
| `WaveDetailPanel.swift` | ~30 |
| `WaveRunsTab.swift` | ~18 |
| `WaveRow.swift` | ~11 |
| `WaveSidebar.swift` | ~10 |
| `QuickExperimentView.swift` | ~11 |
| `SetupView.swift` | ~9 |
| `InteractiveSessionView.swift` | ~7 |
| `StepRunner.swift` | ~5 |

## Risk

Custom fonts must actually be bundled for the Typography calls to take effect. If fonts aren't in the app bundle, the calls silently fall back to system fonts — which is what happens now. Font bundling is the prerequisite.

## Source

Full details in `reports/viz/design-audit.md`, Deviation 2 and Tier 2.
