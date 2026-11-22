# Loopflow Design

One design system for the website, Loopflow for Mac and Loopflow for iPhone.
This file is the system: what Loopflow looks like, what it refuses to look
like, and how a value gets to be part of it. Code that disagrees with this
file is a bug in one of them; fix whichever is wrong in the same change.

[The language](#the-language) is what every surface is moving to.
[Tokens today](#tokens-today) is what the code still ships; the two differ
until the pointers under [What we avoid](#what-we-avoid) are gone.

## What Loopflow is, visually

A terminal with a memory. The terminal pane is the product; everything around
it is there to tell you which work is yours right now and to get out of the
way. The people using it read dense text all day and notice a misaligned
baseline.

- **The work is the picture.** Show a real terminal, a real Task, a real
  command. Never draw a stand-in for the product.
- **Quiet until it matters.** A healthy Session has no status chrome. Color,
  motion and weight are spent only on a change someone has to act on.
- **Dense, then legible.** Tool density, not marketing density. Tighten until
  it scans, then stop before anything truncates.
- **One idea per surface.** Each screen has one thing worth remembering and
  everything else is plain.
- **Words do the work.** A status is a word first and a color second. A label
  that repeats its neighbor is deleted.
- **Fast is part of the look.** Under 100 ms to respond; nothing animates that
  did not change.
- **Every state offers its next step.** "Connect Linear", not "Linear
  unavailable".

## The language

Three voices. Serif is what a person named or asked for, sans is the
interface, mono is the machine.

| Role | Face | Where |
|------|------|-------|
| Serif | Old Standard TT 400 | 24 pt and up only: page headlines, a Wave's or dashboard's name, a Task's title at the top of its Brief, "What do you want?", the title of an empty or failed state |
| Sans | Atkinson Hyperlegible Next 400/600 | Everything read or pressed |
| Mono | Spline Sans Mono 400/700 with italics | Commands, paths, IDs, SHAs, Graph and check names, diffs, the terminal |

Old Standard under 24 pt reads as a typeset paper, so nothing smaller is
serif. Wherever it is set, turn off common ligatures (its "fl" joins in
"Loopflow" and "workflow") and track it at −3%. One serif thing per screen.

The ramp, in points on the apps and pixels on the web:

| Step | Face | Size | Use |
|------|------|------|-----|
| `display` | Serif | 34 | Wave name, "What do you want?"; up to 54 for a web headline |
| `title` | Serif | 26 | Task title in the Brief, dashboard name, state titles; up to 38 for a web section heading |
| `lede` | Sans 600 | 17 | A document's first heading; the text you type in New Task |
| `text` | Sans 400 | 13 | Everything else (16 for web prose) |
| `textStrong` | Sans 600 | 13 | The selected row's title, a pane's name |
| `meta` | Sans 400 | 11 | Counts, times, secondary facts (12 on the web) |
| `mono` | Mono | 12 | Inline IDs, paths, commands |
| `terminal` | Mono | 13 | Terminal cells |

The terminal defaults to the app's mono at the `terminal` step, with
ligatures off and 10% more cell height. A person's Ghostty config overrides
all of it.

Shape names the kind of thing: a control is radius 4, a row 8, a pane 12, a
floating layer 16. Color is the theme's roles, unchanged.

One component for each of these, on every surface that shows it:

- **Task row.** The title in full, then the ID and the status word, with the
  time the status began in tabular digits. A row that needs someone adds one
  sans line saying what it is asking. No stripe, no border, no summary;
  selection is the theme's selection fill. A dashboard tile's header is this
  row.
- **Status.** One dot and a phrase in sans, in the role's color, with the
  next step beside it as a button ("Blocked, a dialog is open", "Go to
  dialog").
- **Workflow line.** The Graph's name in mono, every step as a word, a person
  icon before the steps that stop for someone, the running step strong in the
  working color, and a sentence: "build is running. You are next needed at
  human-review." Where the line does not fit, the sentence stands alone.
- **Button.** 24 pt high, 13 pt text, radius 4, neutral. One accent button
  per surface, for what the surface is for.
- **Empty or failed state.** Left-aligned: a serif title saying what
  happened, one sentence, then what to do next as buttons. No icon over a
  centred title.
- **Title bar.** Sidebar toggle, back, forward, then the path as a jump bar.

`design/prototypes/` holds the reference for layout and wording until the
code matches: `desktop.html` and `desktop-more.html` (Loopflow for Mac, live,
in all 24 themes), `elements.html` (the Task row, workflow line and New Task
three ways each) and `website-fonts.patch` (the website in these faces;
`git apply` it).
Delete a prototype when its screens ship.

## What we avoid

Each pattern below reads as generated rather than designed. The first group is
what Loopflow does today; the pointers say where, so the first cleanup has a
map. Remove a pointer when its code is fixed.

### We do these now

**Small tracked uppercase labels.** No eyebrow above a heading, no caps
section label, no caps kicker, no caps "or". A section is named by a heading
in sentence case or not at all. The one allowed use of capitals is an acronym.
- Desktop builds it into the system: `WorkSectionHeading` and
  `WorkDisclosureHeading` (`swift/LoopflowMac/Design/WorkStyle.swift`), plus
  four hand-made variants (`WaveDetailPane.swift`, `WorkActivityView.swift`,
  `WorkNavigator.swift`).

**Monospace as decoration.** Mono is for things a person could paste into a
terminal: commands, paths, IDs, SHAs, diffs, and numbers in a column. It is
not for prose, section titles, links, captions or buttons.
- Desktop: the sidebar Task summary is prose set in mono; sidebar section
  titles and counts, "Page 1 of N", "builtin Graph".
- Website: the "Markdown source" and "Public user docs" links in the docs;
  agent names and statuses in the homepage's app drawings.

**A stripe down the side of a rounded row.** A colored leading edge means
status and nothing else. A row with no status has no stripe.
- Desktop: every sidebar Task row draws one, falling back to a border color
  when there is no status (`WorkNavigator.swift`).

**Drawings of the product.** No hand-built HTML or SVG picture of an app
window, a dashboard or a team. Show a screenshot or a recording of the real
thing, or show nothing. "Coming soon" over a drawing is two problems.
- Website: both hero figures (`.appmock.duo`), with invented teammates in
  initials circles, a diagonal stripe wash and a rotated dashed stamp.

**Values outside the scale.** See [How the system works](#how-the-system-works).
- Desktop: 20 font sizes against a ramp of 6, half-point sizes 24 times, 62
  calls under 11 pt with a floor of 8 pt; radius 6 is the most used radius and
  is not in the scale; 166 literal paddings.
- iPhone: `Spacing` and `CornerRadius` are never used; 49 of 50 type calls
  pass their own size.
- Website docs and download pages: their own sizes, radii and a second
  palette (`--d-*`). The homepage and studio pages use `:root` steps for
  size, space and radius; 18 raw colors remain there (tints, shadows and the
  avatar colors in the drawings), and several rules still override each other
  with `!important`.

**Two of the same thing.** One component per job, on every surface.
- The website has two navigation bars (homepage and docs) with different
  wordmark casing, links and button shape, and the docs still set Inter.
- Desktop has three workflow renderers, two Create Wave forms and two status
  dots (flat circles and a glass "lens" with a glow).
- iPhone repeats one primary button by hand eight times in two shapes.
- The site sets Instrument Serif and Inter; the apps bundle Cormorant
  Garamond and Lato.

**Tiny text.** Nothing a person is meant to read is under 11 pt on Desktop,
12 pt on iPhone or 12 px on the web.

**Truncated meaning.** If a row's ID, status or owner ends in "…", the row is
carrying too much. Drop a field; don't clip three.
- Desktop sidebar rows: "LOO-515 Block… #1949 Jack…".

**Motion on a loop.** Nothing moves unless it changed. No rotating headline
word, no pulsing dot beside a static label.
- Website: the hero's cycling word.
- Desktop's one perpetual animation, the running Graph node, is the model: it
  appears only while the thing runs and stops under Reduce Motion.

**Text standing in for a demo.** A section that explains the product in six
paragraphs beside a decorative card is a demo made of text. Show one real task
going from start to finish and caption it.

**Em-dash cadence.** Use a full stop, a comma or parentheses. One dash per
page at most.

**Glyphs as icons.** "⚠", "✓", "✗" and "↗" typed into strings sit beside SF
Symbols. Use the icon set, or a word.

### We don't do these; keep it that way

- Purple or blue gradients, gradient text, neon or glow on dark.
- Glass and blur as decoration. Blur is for a layer that sits over live
  content (the QR scanner caption over the camera is the model).
- Cards inside cards; a border and a wide shadow on the same surface.
- Grids of identical icon, heading and sentence cards; icon tiles.
- Hero metrics, logo walls, testimonials, anything nobody can check.
- Emoji as icons.
- A pill or badge above a headline; "New", "Beta", "AI".
- Bounce or elastic easing; entrance animations on scroll.
- Marketing verbs ("supercharge", "seamless"), "Not X. Y." slogans, numbers
  spelled as words.
- A landing-page hero inside an app. An app's first screen is the first task.

### Watch for

Serif display over a warm cream ground is the current default for "tasteful",
the way purple gradients were three years ago. Loopflow's burgundy and its
musical vocabulary are its own; a cream page with an italic serif headline is
not. Whatever the next language is, it has to be recognizable with the logo
covered.

## How the system works

1. **One scale per property, and only named steps.** Type, spacing, radius,
   line width, opacity and duration each have a short list. A view names a
   step (`Typography.meta`, `Spacing.md`, `--fs-small`); it never passes a
   number. Helpers that accept any size are how the scale leaked, so they go.
2. **A new value is a change to the system.** If no step fits, add or move a
   step here and in the token source, in the same change, with the reason.
   Half-point sizes mean the ramp is wrong.
3. **Tokens are roles, not colors.** Views ask for `palette.failure`, never
   `.red`. Selection, keyboard focus, link, action and status are separate
   roles so a theme can change one without the others.
4. **Tokens have one source per platform and the same names on each.**
   Swift: `swift/Loopflow/Design/`. Web: the `:root` block of
   `website/static/style.css`. A token unused for a release is deleted.
5. **One component per job.** A second button, heading, status mark or
   workflow diagram is a bug even when it looks better. Improve the first.
6. **Shape carries meaning.** Radius, border and elevation distinguish kinds
   of thing: a control, a row, a pane, a floating layer. Things that differ in
   kind differ in shape; most things have no shadow.
7. **State is never color alone.** Pair it with a word or a shape. Unknown is
   shown as unknown, not as idle.
8. **Measured, not asserted.** A claim in this file that can be checked has a
   check: palette contrast in `WorkPaletteTests`, site accessibility in
   `website/tests/`. Token discipline should get the same.
9. **Real content.** Design against the longest Task title, the slowest
   Machine and an empty repository, not placeholder text.

## Tokens today

What the code ships now. Where surfaces disagree, the disagreement is listed,
not resolved.

### Type

| Role | Mac and iPhone | Website |
|------|----------------|---------|
| Serif | Cormorant Garamond 400/500/600 | Instrument Serif 400 |
| Sans | Lato 400/700 | Inter 400–700 |
| Mono | JetBrains Mono 400 | JetBrains Mono 400/500 |

Request only bundled weights in the apps; semibold or medium on Lato is
synthesized and renders heavy.

Desktop ramp (`Typography` in `swift/Loopflow/Design/DesignSystem.swift`):

| Step | Face | Size | Use |
|------|------|------|-----|
| `display` | Cormorant 500 | 34 | Wave page title |
| `title` | Cormorant 500 | 26 | Task page title |
| `lede` | Cormorant 500 | 17 | Wave objective |
| `text` | Lato 400 | 13 | Everything else |
| `textStrong` | Lato 700 | 13 | Selected row titles, Session names |
| `meta` | Lato 400 | 11 | Meta, ranks |
| `mono` | JetBrains Mono | 12 | Commands, IDs, paths |

iPhone has no ramp of its own yet; it uses 12, 13, 14, 15, 16, 17, 24 and 34.

Website ramp for the homepage and studio pages (`:root` in
`website/static/style.css`):

| Step | Size | Use |
|------|------|-----|
| `--t-display` | 34 to 60 | Page headline, closing line (serif) |
| `--t-title` | 30 to 42 | Section heading (serif) |
| `--t-name` | 34 | A product's name on the studio page (serif) |
| `--t-head` | 24 | Subheading (serif) |
| `--t-intro` | 21 | Studio page introduction |
| `--t-lede` | 18 | The sentence under a heading |
| `--t-text` | 16 | Prose |
| `--t-small` | 13 | Captions, navigation, terminals |
| `--t-meta` | 12 | Labels, text inside the drawings |

The docs keep their own `--fs-*` ramp.

### Color

Apps: `LoopflowPalette` roles, supplied by each of the 24 bundled themes and
any imported Ghostty theme. Every role is required; a theme missing one fails
to decode. Ordinary text reaches 4.5:1 and meaningful indicators 3:1 on every
surface, including selection fills, in every theme.

Website (`:root`): `--burgundy #722f37` for headings; `--wine #9B1A4A` for the
primary button; `--text #1a1a1a`, `--text-secondary #555555`, `--bg #ffffff`,
`--border #e5e5e5`; `--paper-warm`, `--border-warm`, `--rule-strong`,
`--track` and `--ok` for the warm cards; `--term-*` for the terminal block.
The logo's cyan `#0ab3cc` is `--info`.

### Space, shape, motion

| Property | Steps |
|----------|-------|
| Spacing | 2, 4, 8, 12, 16, 20, 24, 32 (`Spacing.xxs` … `xxxl`) |
| Radius | 4, 8, 12, 16 (`CornerRadius.sm` … `xl`) |
| Hit target | 24 Desktop, 32 comfortable, 44 touch |
| Duration | 100 ms hover and chevrons, 160 ms selection and panels, 240 ms sidebar and splits |

The website names its steps by value: `--space-4` through `--space-96`
(4, 8, 12, 16, 20, 24, 32, 48, 64, 96) and `--radius-sm`, `-md`, `-lg`
(4, 8, 12).

A child's radius is at most its parent's radius minus the padding between
them. Animate only offset, scale, opacity and rotation, through
`DesignAnimation`, which returns nothing under Reduce Motion.

## Desktop semantics

Select Work: the theme’s solid selection fill and contrasting text mark the destination,
including its Needs you entry. File selection keeps its neutral tint and edge.
Tab to a control: the theme’s separate focus outline marks keyboard focus, including
on a selected row with an error. Ordinary controls use neutral ink; only an explicitly
emphasized action uses the theme’s accent. Repo identity uses emphasis ink. Links use
secondary ink and underline on hover/focus in navigation and control chrome.

| Role | Source / behavior |
|------|-------------------|
| Structure and text | `background`, surface ladder, `text`, `textSecondary`, `textTertiary` |
| Work selection | `workSelectedFill`, `workSelectedText`, `workSelectedSecondaryText`; independent of failure |
| File and option selection | `selectionFill`, `selectionEdge` |
| Keyboard focus | `keyboardFocus`; visible with selection |
| Links | `linkInk`, `linkHover`; separate from status |
| Actions | `actionInk`, `actionHover`; `accentInk` for emphasized actions; `emphasisInk` for headings |
| Program activity | `working`, `waiting`, `failure`, `success`; labels and shapes retain state meaning |

Unknown and client-liveness-only observations have no activity label or dot. Shared
Waiting remains explicitly inferred; reported Blocked is not renamed Waiting.
Program Done is not Task completion. Workflow position uses neutral marking; a
stopped edge alone is not a red error. Read failures remain inspectable.

`ThemeDefinition` owns explicit sRGB terminal colors and the semantic
`LoopflowPalette` used by app and pane chrome. Ghostty receives a normalized color
overlay after personal non-color configuration. Retained surfaces update in place.
Each bundled/imported record supplies every palette role; missing roles fail decoding.
`GhosttyThemeMapping` converts imports once: ANSI 4/3/1/2 supply accent/Working,
Waiting, failure and success colors. Work selection is calibrated separately.
Application text/surfaces adjust for readability without altering terminal ANSI.
Ordinary app text is desaturated independently; terminal syntax is preserved.
State selects the role; color never invents activity. Focus aliases accent ink,
links/actions alias text roles, and status stroke/text alias status ink.
Solid controls have distinct base/hover/pressed fills. Ghostty owns terminal cell
fills and contrast; the app owns gutter/header chrome.

Built-ins and imported Desktop appearances have explicit roles. `WorkPaletteTests`
resolves actual SwiftUI colors and composites selection fills over each surface: ordinary text
must reach 4.5:1, meaningful indicators 3:1.

## Work tracking

The sidebar presents the repository's retained working Task order, then direct Wave
rows. Opening started, unpaused Work appends once. Activity and query refresh never
reorder slots; explicit pause hides a slot without deleting its stored position.
Working references are saved through WorkCache; they convey no execution authority.
Unstarted Tasks show their brief beside a Start offer in the Session slot without
joining the working set. Only Start runs anything.

The navigator's Working tasks / Available / Running choice filters presentation
without selecting Work or changing panes, clients, focus or drafts. Missing planning
keeps an unavailable row. Work observation retains its last reading on failure.
Wave navigation leaves working positions and the sidebar viewport intact.

Title-bar breadcrumbs follow hierarchy as a jump bar: each name navigates, and the
› after it lists what is inside (repository › Waves, Wave › Tasks) with a filter that searches that scope. Back follows
visits, and Cmd+K routes
cross-repository Work to its own window. Cmd+1–9 and Control+Tab address the same
stable eligible slots, independent of the filter. Each Work owns one retained
workspace. Dashboard queries compose those workspaces side by side; ordinary
navigation selects one destination. Rust owns query membership and arrangement,
including hidden occurrences and duplicate suppression. Tiles show the primary
Session or explicit zoom without acquiring a provider.

## Text

- Curly quotes and apostrophes, the ellipsis character, a non-breaking space
  between a number and its unit.
- Digits, not words: 3 Tasks.
- `.monospacedDigit()` for counts, times and sizes that change in place.
- One voice for one event: the same failure reads the same on every surface.

## Before merging a UI change

- [ ] Every size, space, radius, color and duration is a named step.
- [ ] No uppercase label, no mono prose, no stripe without a status.
- [ ] Nothing meant to be read is under the size floor, and nothing
      meaningful truncates at the narrowest supported width.
- [ ] The longest real title, an empty state, a loading state and a failure
      each have a design, and each failure offers its next step.
- [ ] Keyboard reaches it; focus is visible on a selected row.
- [ ] Reduce Motion leaves a complete, still screen.
- [ ] Icon-only controls have an accessibility label; touch targets are 44 pt.
- [ ] Checked in light and dark, and in one non-default theme.
- [ ] Looked at on the real surface, not only in source.
