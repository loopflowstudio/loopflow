-- Desktop pane layouts become three designated panes: session, shell and browser.
-- Released free trees (empty, session, shell, files, log) are not converted (Jack
-- Heart, 2026-10-25: existing layouts may reset); Desktop rebuilds the default.
-- breaks: released Desktop reads Empty, Files and Log panes, which this shape no longer writes
DELETE FROM work_pane_layouts;
