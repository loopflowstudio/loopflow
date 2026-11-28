# LOO-1126 open questions

Review choices adopted headless are listed in `plan.md` under "Design
decisions made for review" (second review Q1–Q5).

Still open:
- Sessions started from Rex lose `TERM_PROGRAM=rex`. Is any Rex-specific
  behavior inside a Session wanted?
- Claude's `file://` link shape (line fragment or not) wasn't confirmed from
  its binary. The classifier keeps `#L<n>` and refuses other fragments.
- Codex's hyperlink detection was not probed.
- Relative text links: Ghostty can make a regex match for a relative path
  absolute after a local existence check. Confirm the configured matches
  before promising relative links in remote panes.
- Hover has no link kind, so `TerminalLink.preview` classifies with bare
  paths allowed; an OSC 8 bare path previews but a click refuses it.

Pass 1 choices:
- Keep web fragments such as GitHub discussion anchors. The `#L<n>` rule
  applies to file links.
- Refuse web URLs with credentials.
- Use a confirmation sheet for external URL schemes.
- Enable previews for detected text links as well as OSC 8 links.
- Refuse parent-directory segments in terminal file URLs.
