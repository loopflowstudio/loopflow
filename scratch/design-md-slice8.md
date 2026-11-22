# Slice 8: DESIGN.md edits, prepared against origin/main 4cee2ed182

Apply after the sync brings root `DESIGN.md` in. Read from the branch on
2026-11-21; re-run the greps below if more commits land first.

## "We do these now" pointers

- **Small tracked uppercase labels.** `WorkSectionHeading`/`WorkDisclosureHeading`
  are sentence case `textStrong` (slice 1). Still uppercase:
  `WaveDetailPane.swift:408` (`.textCase(.uppercase)` + `.tracking(0.6)`),
  `WorkActivityView.swift:109` (`kind.rawValue.uppercased()` + tracking).
  None left in `WorkNavigator.swift`. → Replace the bullet with
  "Desktop: two hand-made variants (`WaveDetailPane.swift`, `WorkActivityView.swift`)."
- **Monospace as decoration.** Sidebar summary gone (slice 5); section titles
  and counts sans. Still mono: "Page 1 of N" (`DashboardSlotGrid.swift:144`),
  "builtin Graph" (`WorkPalette.swift:380`). → keep those two, drop the rest
  of the Desktop clause.
- **Stripe down the side.** Gone (slice 5, `TaskRow`). → remove the pointer.
- **Values outside the scale.** Desktop: no sized helper, no literal font
  size (`TypographyDisciplineTests`), radius on `control/row/pane/floating`.
  Literal paddings: recount with
  `git grep -c -E '\.padding\((\.[a-z]+, )?[0-9]' -- swift/LoopflowMac | awk -F: '{s+=$2} END {print s}'`.
  iPhone: `MobileType` (`swift/LoopflowiOS/MobileType.swift`) keeps the old
  sizes in the new faces until the iPhone Task. → rewrite the Desktop line
  to the padding count only; iPhone line names `MobileType`.
- **Two of the same thing.** Workflow renderers: `WorkflowLine`
  (`LoopflowMac/Design/WorkflowLine.swift`) and New Task's `WorkflowSteps`
  (`NewTaskSheet.swift:188`); `WorkflowDiagram.swift` and the bar's dots are
  gone. Status dots: one, `StatusMark`; `WaveLensView` gone. Create Wave
  forms: not touched, check before editing. Fonts: apps bundle Old Standard
  TT, Atkinson Hyperlegible Next, Spline Sans Mono; check what #2011 says the
  site sets.
- **Truncated meaning.** `TaskRow` wraps the title, then ID and status word.
  → remove the sidebar pointer.
- **Glyphs as icons.** No `↗` left. Still typed: `✓`/`✗` in
  `BacklogView.swift:396`, `⚠` in `TaskFilesView.swift:366`. → point there.

## "Tokens today"

- Type table, Mac and iPhone column: Old Standard TT 400 / Atkinson
  Hyperlegible Next 400, 600 (+ italics) / Spline Sans Mono 400, 700 (+
  italics). Replace the Lato synthesis sentence: request bundled faces by
  PostScript name (`Typography.*Face`).
- Desktop ramp table (`swift/Loopflow/Design/DesignSystem.swift`):
  `display` Old Standard 34 (`.serif(.display)`), `title` Old Standard 26
  (`.serif(.title)`), both −0.03 em tracking and ligatures off, applied only
  through `.serif(_:)`; `lede` Atkinson 600 17; `text` Atkinson 400 13;
  `textStrong` Atkinson 600 13; `meta` Atkinson 400 11; `mono` Spline Sans
  Mono 12; terminal Spline Sans Mono 13 (`Typography.terminalSize`, Ghostty
  draws it). Checked by `TypographyDisciplineTests`.
- iPhone line: `MobileType` holds 12–24 in the new faces until it has a ramp.
- Radius row: `4, 8, 12, 16 (CornerRadius.control, row, pane, floating)`.
- Add a Components paragraph: `TaskRow`, `StatusMark`, `WorkflowLine`,
  `LoopflowButtonStyle(.neutral|.accent|.destructive)`, `TitleBar`, all in
  `swift/LoopflowMac/Design/`; the workflow line's steps and sentence come
  from Rust (`TaskGraph.steps/current_step/sentence`).
- Principle 8 "Token discipline should get the same" → it has one now:
  `TypographyDisciplineTests`.

## After applying

`git grep` each backticked path in DESIGN.md; run
`scripts/test_desktop.sh --filter 'TypographyDisciplineTests|WorkPaletteTests|WorkflowLineTests'`
and `cargo test -p loopflow --test integration documented_commands::`
(it checks design documents).
