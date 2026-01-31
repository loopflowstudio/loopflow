# Concerto Backlog Restructure

Transform `roadmap/concerto-next/` into a pure backlog. Move design docs to `reports/`, personas to `.lf/directions/`.

## Status: Complete

The restructure is done:
- `roadmap/concerto/` — pure backlog with Phase 2/3 items
- `reports/concerto/` — design docs (00-09)
- `.lf/directions/` — conductor, improviser, returner personas
- `.lf/steps/ux-review.md` — screenshot-driven review step
- `scripts/screenshots.yaml` — screenshot manifest

## Screenshot pipeline

```bash
# Generate screenshots
python scripts/generate_screenshots.py

# Review all with each persona
lf ux-review --direction conductor --area docs/screenshots/
lf ux-review --direction improviser --area docs/screenshots/
lf ux-review --direction returner --area docs/screenshots/
```

Screenshots in `docs/screenshots/`:
- `concerto-main.png` — sidebar with grouped waves
- `concerto-wave-running.png` — running wave detail
- `concerto-wave-waiting.png` — waiting wave detail

The step reads all images in the area, applies persona questions, outputs backlog items.

## Backlog structure

```
roadmap/concerto/
├── README.md
│
├── # Phase 1: Polish (discovered via persona review)
├── dashboard-grouping-clarity.md
├── connect-click-count.md
├── ...
│
├── # Phase 2: Remote access foundation
├── waveservice-protocol.md
├── grpc-terminal-streaming.md
├── loopflow-auth.md
├── lfd-registration.md
│
├── # Phase 3: Mobile
├── ios-conduct-ui.md
├── ios-improvise-ui.md
├── remote-terminal-view.md
├── push-notifications.md
```

Each item:

```yaml
---
status: todo | in-progress | done
phase: 1 | 2 | 3
persona: conductor | improviser | returner  # who this serves (optional)
screenshot: docs/concerto-main.png  # evidence (optional)
---
```

```markdown
# Dashboard Grouping Clarity

Conductor can't tell "needs me" from "running" at a glance.

## Current
WaveSidebar groups: "Needs Attention", "Open PRs", "Active", "Idle"

## Problem
"Needs Attention" only shows errors. Interactive-waiting waves are in "Active".
Conductor has to scan Active to find what needs them.

## Build
- Add "Waiting for You" section for interactive-waiting waves
- Or: visual distinction within Active for waiting vs running

## Evidence
[screenshot showing the issue]

## Done when
Conductor question passes: "Can I tell what needs attention without clicking anything?"
```

## Personas as directions

Personas become `.lf/directions/` files—a lens that applies to any step and any area. See PROMPT_STYLE.md § Orthogonality.

```markdown
# .lf/directions/conductor.md

Managing multiple parallel workstreams. Checking in, not diving deep.

- Can I see what needs attention without drilling in?
- Is urgency visually obvious?
- How many clicks from "I see a problem" to "I'm acting on it"?
- Would I trust this to surface the right thing while I'm away?
- Do routine actions feel like single actions or workflows?
- Am I confirming things I already decided?

Red flags:
- Flat lists with no hierarchy
- Status requires interaction to reveal
- Confirmation dialogs for routine operations
```

```markdown
# .lf/directions/improviser.md

Exploring unfamiliar territory. Doesn't know the right approach yet.

- How fast from intent to action?
- Am I configuring things I don't care about yet?
- Can I do one thing without committing to a sequence?
- Does this feel like a workshop or a form?
- If I change my mind, is that cheap?
- Am I exploring or filling out paperwork?

Red flags:
- Setup wizards before first action
- Required fields for optional concepts
- Changing course requires starting over
```

```markdown
# .lf/directions/returner.md

Was away. Needs to catch up and triage.

- Can I tell what happened while I was gone?
- Is "needs me" vs "fine" instant to distinguish?
- Do I have to click into each item to understand state?
- Is there a summary or must I reconstruct context?
- Can I triage N items in a few minutes?

Red flags:
- No history/timeline visible
- State only visible after navigation
- Context lost between sessions
```

## UX review step

Screenshot-driven persona review that generates backlog items.

```markdown
# .lf/steps/ux-review.md
---
requires: screenshot
produces: backlog items
interactive: true
---

Review the screenshot through the lens of the given direction (persona).

Apply each question from the direction. For each question:
1. Can the current UI answer it positively?
2. If not, what's the friction?
3. What would fix it?

Output backlog items in this format:

    ---
    status: todo
    phase: 1
    persona: <direction>
    screenshot: <screenshot-path>
    ---

    # <Title>

    <One-line problem statement>

    ## Current
    <What exists now>

    ## Problem
    <Why it fails the persona question>

    ## Build
    <What to change>

    ## Done when
    <Persona question that should pass>
```

Usage:

```bash
lf ux-review --direction conductor --area docs/concerto-main.png
lf ux-review --direction improviser --area docs/concerto-main.png
lf ux-review --direction returner --area docs/concerto-main.png
```

## Phase 2 items (remote access foundation)

From `reports/concerto/01-platform.md` and `02-auth.md`:

| Item | Description |
|------|-------------|
| `waveservice-protocol.md` | Abstract transport so same UI works against Python lfd and future Rust lfd |
| `grpc-terminal-streaming.md` | Bidirectional stream for remote terminal I/O |
| `loopflow-auth.md` | GitHub OAuth for remote access |
| `lfd-registration.md` | lfd registers with Loopflow, receives tokens |

## Phase 3 items (mobile)

From `reports/concerto/09-phasing.md`:

| Item | Description |
|------|-------------|
| `ios-conduct-ui.md` | Same Conduct dashboard on iOS/iPad |
| `ios-improvise-ui.md` | Same Improvise flow on iOS/iPad |
| `remote-terminal-view.md` | Terminal view streaming from lfd |
| `push-notifications.md` | APNS integration via Loopflow |

## Constraints

- Phase 1 items come from persona+screenshot review
- Phase 2/3 items come from design docs in `reports/concerto/`
- Each item has a phase, optional persona
- Personas are directions in `.lf/directions/`

## Done when

```bash
# Structure
ls roadmap/concerto/*.md           # backlog items (phases 1-3)
ls reports/concerto/*.md           # design reference
ls .lf/directions/conductor.md     # persona directions
ls .lf/steps/ux-review.md          # screenshot review step

# Can generate Phase 1 items
lf ux-review --direction conductor --area docs/concerto-main.png

# Can ingest from backlog
lf ingest  # picks from roadmap/concerto/

# Personas work
lf review --direction conductor
```
