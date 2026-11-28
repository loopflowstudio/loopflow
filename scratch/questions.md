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

Pass 1 choices:
- Keep web fragments such as GitHub discussion anchors. The `#L<n>` rule
  applies to file links.
- Refuse web URLs with credentials. A preview like
  `https://example.com@other.example/` can mislead about the destination.
- Use a confirmation sheet for external URL schemes, following Desktop's
  confirmation pattern.
- Enable previews for detected text links as well as OSC 8 links, so both
  use the terminal's target overlay.
- Refuse parent-directory segments in terminal file URLs. Compare checkout
  paths lexically so a remote filename never triggers a lookup on this Mac.
