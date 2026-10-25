-- Desktop pane layouts become three designated panes: session, shell and browser.
-- convert_released_data recognizes this statement and rewrites every layout_json
-- (store::sqlite::selections::convert_released_layouts): Files and Log panes merge
-- into one browser pane, empty and extra session panes leave the layout, and
-- missing designated panes are added hidden. File drafts are kept unchanged.
-- breaks: released Desktop reads Empty, Files and Log panes, which this shape no longer writes
UPDATE work_pane_layouts SET layout_json=layout_json WHERE 0;
