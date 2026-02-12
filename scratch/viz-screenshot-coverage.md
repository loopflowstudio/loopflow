---
status: todo
phase: 1
---

# Screenshot coverage and persona subdivision

Current manifest (`scripts/screenshots.yaml`) covers 3 states: main dashboard, running wave, waiting wave. Missing: improvise mode, empty state, error states.

## Gaps

The persona directions ask questions that current screenshots can't answer:

**Conductor:**
- "Would I trust this to surface the right thing while I'm away?" — no history/timeline screenshot
- "Do routine actions feel like single actions?" — no action flow screenshots

**Improviser:**
- "How fast from intent to action?" — improvise mode screenshot is commented out
- "Can I do one thing without committing to a sequence?" — no quick experiment screenshot

**Listener:**
- "Can I tell what happened while I was gone?" — no history view screenshot
- "Is there a summary?" — no summary state screenshot

## Build

- Audit `screenshots.yaml` against persona directions — which questions can't be answered?
- Add entries for missing states: improvise mode, empty state, error states, history view
- Uncomment the improvise mode screenshot (requires `show_launcher` flag in Concerto)
- Add `--direction` flag to `generate_screenshots.py` (default: all)
- Organize output by persona: `docs/screenshots/conductor/`, `docs/screenshots/improviser/`, `docs/screenshots/listener/`

## Done when

`python scripts/generate_screenshots.py --direction conductor` generates conductor-relevant screenshots. Default generates all. Output organized by persona directory.
