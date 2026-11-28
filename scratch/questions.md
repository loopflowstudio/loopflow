# LOO-1126 open questions

The concept review's choices were adopted headless and marked for review in
`plan.md` under "Design decisions made for review":
- checkout files open in the Files pane at their line (review Q1, 1a)
- `TERM_PROGRAM=ghostty` on every held terminal (Q2, 2a)
- read-only Work links open in this app (Q3, 3a)

Still open:
- Sessions started from Rex lose `TERM_PROGRAM=rex`. Is any Rex-specific
  behavior inside a Session wanted?
- Claude's `file://` link shape (line fragment or not) wasn't confirmed from
  its binary. The classifier keeps `#L<n>` and refuses other fragments.
- Codex's hyperlink detection was not probed.
