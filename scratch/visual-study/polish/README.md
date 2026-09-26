# Polish directions for the native workspace

```sh
uv run --no-project python -m http.server 8317 --bind 127.0.0.1
open 'http://127.0.0.1:8317/scratch/visual-study/polish/index.html?surface=task'   # opens D; ?v=a|b|c for the originals
```

Jack asked for three visual polish directions on this branch's current build. Each keeps
the cycle 1–4 structure (repo-headed sidebar with bottom search, Wave objective → KRs
→ Tasks, the Task sequence Flow → status → Sessions → Description → Comments, and
the Wave / Task / Session breadcrumb) and changes only spacing, type, color, rhythm,
state treatment and density. `?v=a|b|c|d` picks the direction (D is the default) and `?surface=wave|task|session`
picks the surface. Both also have buttons in the study bar.

## What each direction changes

| Rough edge (native captures) | A · Quiet editorial | B · Crisp tool | C · Warm spacious |
|---|---|---|---|
| Raw warning dominates the Wave | One line with an ochre left rule; **Details** discloses the full reason and a copyable recovery command; **Dismiss** hides it for this visit | Compact tinted bar, same disclosure | Soft card, same disclosure |
| Headings are inconsistent (small caps vs serif "Metrics") | One system: 11.5px tracked caps with a hairline | One system: 11px caps, no rule | One system: Cormorant 25px |
| Metrics outweigh Tasks | Hairline text block after Tasks | Single table row | The only soft card on the page, because it carries a reading |
| Chapter history link floats | Right side of the Current KRs heading (all directions) | same | same |
| Heavy selected pill in sidebar | Light tint plus a 2px burgundy edge mark | Flat tinted rectangle, denser rows | Hairline outline with burgundy text |
| Cramped Session count, weak header glyph | Fixed count column with bubble icon; drawn filter glyph with a hover state | Mono count pill | Same as A, more air |
| Flow tail off-screen at ~1100pt | Three rows: once, loops, then a quieter tail row, as an indented staircase | Two rows at ≥1240px with numbered 11px chips and a `↳ delivery` tail; three rows below that | Three rows with serif labels (Once / Loops / Then) and larger chips |
| Return arrowheads collide at implement | Loop 2 lands left of centre on a lower lane, Loop 1 right of centre on a higher lane, so the lines never cross (all directions) | same, tighter lanes | Widest lanes; Loop 2 uses an open arrowhead |
| Stacked duplicate "Loop · Iteration 3" | Each loop labels its own count ("Loop 1 · 2 returns" top-left, "Loop 2 · 1 return" top-right), and the header shows **Iteration (2, 1)** (all directions) | Header tuple as a mono chip | same as A |
| Completed green reads grey under the loop tint | Nodes have an opaque base, so state colors keep their hue inside tinted regions (all directions) | Slightly stronger fills | same as A |
| Task page rhythm | 34px section gap, hairline section heads | 22px gap; Flow and Sessions are bordered panels | 48px gap |

Shared states: blue for running and loops, green for completed (including completed
human steps), yellow for a pending human step, red for blocked, and neutral dashed for
stopped. Skill names are literal lowercase mono. There is no Pause. Resume appears only
for a stopped Flow, Start only for an unstarted one, and **Stop & restart…** shows on hover
or focus of the Flow name and opens a confirmation.

## Working locally

- Task states: LOO-291 is running (two Sessions), LOO-293 is stopped at review-slice,
  LOO-285 (infrastructure) is blocked at gate, and plan-only Tasks are unstarted previews.
- A Task row opens its only Session directly; with zero or several Sessions it opens the
  Task overview. The count pill always opens the overview.
- Click a node for its detail. Escape closes the detail or the restart confirmation.
- The Session membership chip returns to the Task and highlights that exact node, which
  prototypes the missing chip → graph interaction.
- Session drafts are kept per Session. Rename works through the pencil (Enter saves,
  Escape cancels).
- Search filters the sidebar. You can collapse a Wave, and expand Description, Comments
  and notice Details.

## Limits

- This is sample data in a website. It does not show native rendering, Ghostty panes,
  focus ownership or performance.
- Loop regions and arrows are measured from the rendered DOM. A native implementation
  needs its own layout, driven by the pinned definition rather than this fixture's row
  breaks. Row breaks here assume both loops share a row, which is true of Feature today.
  A Flow whose loop spans a row break would need a different rule.
- The per-loop labels count returns, following Jack's tuple decision. The runtime tuple
  projection is still pending ([jack-iteration-tuple.md](../../implementation-cycle/jack-iteration-tuple.md)).
- Comments and recent Runs are fixtures. The native comment read does not exist yet.
- Captures at 1440×900 and 1100×800 were inspected for all three directions. Neither
  size scrolls the page horizontally. No human preference has been recorded yet.

## Jack's pick — 2026-09-25

"left pane: A, center pane: B, vibes: C". Combine them as direction D (`?v=d`):
- Sidebar exactly as A (light tint and 2px burgundy edge mark, fixed count column, drawn filter glyph).
- Center content structure and density from B (one 11px caps heading system, aligned ID/state columns, Metrics as a table row, bordered Flow/Sessions panels, compact two-row Flow with numbered chips and `↳ delivery` tail, mono tuple chip).
- Overall mood from C (warm cream/burgundy-as-accent palette, softer surfaces, Cormorant for page titles only, more generous outer margins while keeping B's inner density).

### Direction D (`?v=d`, now the default)

D is a scoped combination rather than a fourth stylesheet. In `polish.css`, A's selected-row rules and B's
center-pane rules have the selector `:is(.v-a, .v-d)` / `:is(.v-b, .v-d)`. A short `.v-d` block then adds
C's mood on top. In `polish.js`, `center()` maps D to B for the Flow row layout, numbered nodes, loop geometry
and the metric table.

| From | What D uses |
|---|---|
| A (left pane) | 264px sidebar, light tint with 2px burgundy edge mark on selection, serif Wave names, fixed count column, drawn filter glyph, bottom search |
| B (center structure) | 11px caps headings, aligned ID/state columns in the Task plan, Metrics as one table row, bordered Flow/Sessions panels, two-row Flow ≥1240px with numbered 11px chips and a `↳ delivery` tail, three labelled rows below that, mono `Iteration (2, 1)` chip, compact status/Session rows |
| C (vibe) | Cormorant for page titles only (Wave 48px, Task 32px, objective in serif), outlined burgundy primary button, 12px radii with a soft shadow in place of B's hard panel borders, 12px pane gaps and rounded terminals, 36/44px outer page margins |

Conflicts resolved:

- **Sidebar background**: C uses a cream sidebar, but "left pane: A" wins, so D keeps A's `--side` tone.
- **Notice**: C's soft card versus B's compact tinted bar. D keeps B's bar (structure) with C's softer 10px radius.
- **Metrics**: C's reading card versus B's table row. D keeps the table with C's surface treatment.
- **Width**: A's 264px sidebar plus C's margins pushed B's two-row Flow off the right edge at 1440, and the
  three-row "loops" row off at 1100 (first capture). D widens the page to 1160px and trims outer padding to 44px.
  Below 1240px it also gives back C's margin (24px), tightens the Flow panel padding and the row-label width.
  With those changes, the complete Flow including `pr land -c` fits at both sizes.
- **Task title**: B uses a 21px sans title and C a 30px sans title; Jack's "Cormorant for page titles" makes it serif in D.

Checked with `lf screenshot` at 1440×900 and 1100×800 for Wave, Task and Session in D; each capture was inspected.
Nothing is clipped and there is no page-level horizontal scroll. A/B/C Task captures at 1440 were re-inspected and
still render as before. The limits listed above still apply.
