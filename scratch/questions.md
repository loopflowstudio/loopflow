# LOO-673 open questions

## Session tile hover controls inset (explore, 2026-11-06, headless)
Page: `scratch/explore.html`. Recommended B: one `PaneGeometry.inset` (= corner radius, 8pt) for strip leading/trailing, content reserve and stripless tab rows; today the strip is leading 12 / trailing 4, so zoom/hide sit inside the 8pt corner arc. Implemented B at 8pt (headless, simplest reversible choice); not yet seen by Jack Heart. Concept review: 8pt follows from "matching the pane padding elsewhere" (the terminal reserve is the corner radius). Still open: whether the floating corner pill (C) becomes its own Task.

## Mock Wave fixture rot (compress, 2026-11-06)
`MockWaveFixture.detailJSON` is a hand copy of `wave_detail.json` that no longer decodes (runtime `"status": "ready"` predates this branch); `try?` hides it, so `mock-waves` shows "Fixture missing". Compress only renamed its sections to `TaskSection` values. Open: delete the copy or load the round-tripped fixture — separate Task.
