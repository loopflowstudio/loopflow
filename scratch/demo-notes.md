# LOO-1067 Demo Notes

- **Jack Heart (2026-11-24):** "`'/Users/jack/Desktop/Screenshot 2026-11-24 at 6.31.46 PM.png'` looks a little weird to me"
- **Jack Heart (2026-11-24):** "a little too compact"
- **Jack Heart (2026-11-24):** "what are the commands to navigate tit"
- **Jack Heart (2026-11-24):** "i was looking at tt 2 mostly" (re: #2, the left rail `navigator`)
- **Jack Heart (2026-11-24):** "can you open dev on this workout or something, or manabot or somethign"
- **Jack Heart (2026-11-24):** "cmd click on render_total in main doesnt seme to do anything / maybe i have the wrong one open"
- **Jack Heart (2026-11-24):** "open one mroe time? got some sort of error in the sesson"
- **Screenshot (`scratch/demo-shot-2.png`, 2026-11-24 9:16 PM):**
  1. `TaskFilesStore` on line 21 is blue text-selected (not underlined) — confirms macOS `.textSelection(.enabled)` swallows clicks before `TaskCodeReader`'s `SpatialTapGesture()` fires, breaking both `⌘-click` and click-to-underline (`gd`/`gr`).
  2. Left rail tree (`Checkout` under `swift/LoopflowMac/Views/`) is visibly cramped and over-indented (`depth * 12pt` + doc icon inside 236pt rail), middle-truncating almost every filename (`Configurat...View.swift`, `ReviewStre...View.swift`, `TaskCommen...View.swift`) with 2pt row padding and tight vertical rhythm.
  3. The `The conversation did not open` error banner (`No agent selected`) covered/pushed the pane strip and Files header bar (fixed in demo `LF_HOME` by setting `default_agent: claude`; clicking Retry clears it).
  4. Code reader syntax colors and green gutter mark on line 6 read cleanly, though line spacing is tight.
- **Jack Heart (2026-11-24):** "cmd+click on eg ServedWrite in WorkModel.swift still oes nothing"
- **Jack Heart (2026-11-24):** "CmdShift O, DCmdP work. cmd f weirdly horizontal scrolled ? '/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/TemporaryItems/NSIRD_screencaptureui_7PhjmX/Screenshot 2026-11-24 at 10.18.49 PM.png'"
  - **Root cause found (`TaskCodeReader.swift:88`):** `proxy.scrollTo(line - 1, anchor: .center)` inside a 2D `ScrollView([.horizontal, .vertical])` uses `UnitPoint(x: 0.5, y: 0.5)`, which scrolls horizontally to the horizontal midpoint of the widest line in the file! Should scroll to `UnitPoint(x: 0, y: 0.5)` or the match's horizontal offset (reusing `scrollAnchor`).
- **Jack Heart (2026-11-24):** "`'/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/TemporaryItems/NSIRD_screencaptureui_YsC6QA/Screenshot 2026-11-24 at 10.19.33 PM.png'` a little cramped and not clear at first that the entry box is an entry box an not a header"
  - **Root cause found (`FileFind.swift:106-120`, `TaskFilePicker.swift:197-207`, `TaskFilesView.swift:52-54`):** Both `FileFindBar` and `TaskFilePickerView` render bare `PaletteSearchField` directly on `palette.background` with no input field surface/border (`CornerRadius.control` + `palette.surface` + `palette.borderStrong`) and no divider separating `documentRow` from `FileFindBar`, so the text field reads like a cramped second header row rather than an input box.
- **Jack Heart (2026-11-24):** "`'/Users/jack/Desktop/Screenshot 2026-11-24 at 10.20.18 PM.png'` this treatment oesnt work"
- **Jack Heart (2026-11-24):** "looks fine in Loopflow lgiht,"
