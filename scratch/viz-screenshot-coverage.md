---
status: in-progress
phase: 1
---

# Screenshot coverage and persona subdivision

Screenshots exist for 3 states (dashboard, running, waiting). Persona directions ask questions these can't answer. Fix the coverage and make screenshots queryable by persona.

## Problem

The `lf ux-review --direction conductor` workflow depends on screenshots that illustrate what the persona cares about. Current screenshots miss entire UI states, and there's no way to filter by persona. A conductor review that can't see the runs tab or a listener review that can't see history is answering the wrong questions.

## Approach

Two changes: more screenshots in the manifest, and a `directions` tag on each entry so `generate_screenshots.py --direction conductor` produces only conductor-relevant shots.

No directory reorganization. Screenshots stay flat in `docs/screenshots/`. Persona filtering happens at generation time via tags in `screenshots.yaml`, not via filesystem hierarchy. Subdirectories create maintenance burden (symlinks, duplicates, or broken paths) for zero benefit — the script already knows which shots to generate.

## Coverage gap analysis

### What each persona needs to see

**Conductor** — "What needs attention? Can I act fast?"
| Question | Screenshot needed | Exists? |
|----------|------------------|---------|
| Can I see what needs attention without drilling in? | Main dashboard with grouped sidebar | Yes (`concerto-main`) |
| Is urgency visually obvious? | Waiting wave with status card | Yes (`concerto-wave-waiting`) |
| How many clicks from problem to action? | Running wave with controls | Yes (`concerto-wave-running`) |
| Would I trust this while I'm away? | Runs tab with history + PRs | **No** |
| Do routine actions feel like single actions? | Wave with land/next buttons visible | Partially (in running detail) |

**Improviser** — "How fast from intent to action?"
| Question | Screenshot needed | Exists? |
|----------|------------------|---------|
| How fast from intent to action? | Quick experiment view (no waves) | **No** |
| Am I configuring things I don't care about yet? | Quick experiment detail panel | **No** |
| Can I do one thing without a sequence? | Quick experiment view | **No** |
| Does this feel like a workshop or a form? | New wave creation flow | **No** |

**Listener** — "What happened while I was gone?"
| Question | Screenshot needed | Exists? |
|----------|------------------|---------|
| Can I tell what happened? | Runs tab with completed runs | **No** |
| Is "needs me" vs "fine" instant? | Dashboard with mixed statuses | Yes (`concerto-main`) |
| Do I have to click into each item? | Dashboard sidebar grouping | Yes (`concerto-main`) |
| Is there a summary? | Wave detail with iteration timeline | Partially (in running detail) |

### New screenshots to add

| Name | State | Directions | Mock setup needed |
|------|-------|------------|-------------------|
| `concerto-empty` | No waves, Quick Experiment visible | improviser | No waves in mock |
| `concerto-wave-failed` | Failed wave selected | conductor, listener | New mock wave with `.failed` status |
| `concerto-wave-runs` | Runs tab selected | conductor, listener | Mock wave with runs data + `--tab runs` CLI flag |

### Existing screenshots — add direction tags

| Name | Directions |
|------|-----------|
| `concerto-main` | conductor, listener |
| `concerto-wave-running` | conductor, improviser |
| `concerto-wave-waiting` | conductor, listener |

### Dropped from scope

**Improvise mode launcher** — commented out in current manifest, requires `show_launcher` flag that doesn't exist in Concerto. The Quick Experiment view (no-wave empty state) already answers the improviser's core questions better. If/when a launcher mode exists, add it then.

**Interactive session screenshot** — requires embedded Ghostty terminal running. Too much infrastructure for a screenshot. Not blocking any persona question.

**Command palette** — transient overlay, not a state. Poor screenshot target.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Subdirectories per persona (`docs/screenshots/conductor/`) | Clean filesystem, but screenshots shared across personas need symlinks or copies | Duplicates waste disk; symlinks break on some platforms; filtering at generation is simpler |
| Separate manifests per persona | Each persona gets its own yaml | Duplication — shared screenshots defined 3 times. Single manifest with tags is DRY |
| No persona filtering, just more screenshots | Simpler script, run all every time | `ux-review --direction conductor` gets irrelevant shots. Filtering matters for focused review |

## Key decisions

**Tags, not directories.** Each screenshot entry gets a `directions: [conductor, listener]` field. The script filters by this. Output stays flat. This follows the wave principle that direction is orthogonal to what you're doing — the same screenshot can serve multiple personas.

**Snapshot-only for new screenshots.** UI test captures exist as a slower, more reliable fallback. New screenshots use snapshot type only. UI test variants can be added later if snapshot quality is insufficient.

**Mock data expansion, not new Concerto flags.** The `configureMockWaves()` method gets a failed wave and populated `recentSteps`. No new CLI flags needed — mock data is where screenshot states are defined.

**Empty state via separate mock config.** `concerto-empty` needs zero waves. Add a `mock_config` field to manifest entries (e.g., `mock_config: empty`) and a matching `configureMockWavesEmpty()` in RepoState. Cleaner than a CLI flag for each mock variant.

**`--tab` flag for tab selection.** The runs tab screenshot needs `WaveDetailPanel.selectedTab` set to `.runs`. Add `--tab <name>` to Concerto's screenshot CLI args and pass it through `ScreenshotMode`. The `ScreenshotWindow` sets an environment value that `WaveDetailPanel` reads to override `selectedTab` initial value. Small, targeted change.

## Scope

**In scope:**
- Add `directions` field to `Screenshot` dataclass and manifest entries
- Add `--direction` flag to `generate_screenshots.py` (filters by tag, default: all)
- Add 3 new screenshot entries to `screenshots.yaml`
- Add mock data for failed wave and runs history in `configureMockWaves()`
- Add `configureMockWavesEmpty()` for empty state
- Add `mock_config` field to Screenshot/manifest for choosing mock variant
- Tag all existing entries with appropriate directions
- Add `--tab` flag to Concerto screenshot mode and `select_tab` field to manifest
- Update `screenshots` step to pass through `--direction` if provided

**Out of scope:**
- Launcher/improvise mode (doesn't exist yet)
- Interactive session screenshots (too much infra)
- Directory reorganization
- UI test variants for new screenshots
- Changing the `ux-review` step to auto-filter (it can pass `--direction` manually)

## Build sequence

1. **Manifest + script changes** — add `directions`, `mock_config`, and `select_tab` fields to dataclass, manifest parsing, and `--direction` CLI filtering
2. **Concerto `--tab` flag** — add `tab` to `ScreenshotMode`, pass through to `WaveDetailPanel` via environment
3. **Mock data** — add `configureMockWavesEmpty()` and expand `configureMockWaves()` with failed wave + mock runs
4. **New manifest entries** — add `concerto-empty`, `concerto-wave-failed`, `concerto-wave-runs` to `screenshots.yaml`
5. **Tag existing entries** — add `directions` to all existing screenshot entries
6. **Verify** — `generate_screenshots.py --direction conductor` produces only conductor-tagged shots

## Done when

```bash
# Generates only conductor-relevant screenshots
uv run python scripts/generate_screenshots.py --direction conductor

# Generates all screenshots (backwards compatible)
uv run python scripts/generate_screenshots.py

# New screenshots exist
ls docs/screenshots/concerto-empty.png
ls docs/screenshots/concerto-wave-failed.png
ls docs/screenshots/concerto-wave-runs.png
```
