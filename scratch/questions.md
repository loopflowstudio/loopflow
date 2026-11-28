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

Pass 1 choices:
- Keep web fragments such as GitHub discussion anchors. The `#L<n>` rule
  applies to file links.
- Refuse web URLs with credentials.
- Use a confirmation sheet for external URL schemes.
- Enable previews for detected text links as well as OSC 8 links.
- Refuse parent-directory segments in terminal file URLs.

Pass 2 choices:
- The terminal gives every valid Work address to the Work resolver. A name
  such as `update` can resolve as a registered repository. Terminal clicks
  never dispatch through the app's control-URL handler.
- A move observed by Desktop during lookup stops the Files reveal. File
  reads still follow the Task's active route on the server. QA needs a
  server-first move to check the gap before Desktop observes it.
- An exact Session address with `machine` passes the Work grammar. Desktop's
  existing opener resolves repository root `/` for a remote-only Session and
  prepares Sessions by ID without a Machine selector. Opening remote-only
  standalone Sessions needs its own Desktop routing work.

Pass 4 choices:
- Entering a Task from a file link in a zoomed tile clears that zoom so both
  the Session and Files are visible. It keeps focus on the terminal.

Pass 5 choices (fourth review):
- Reversed Pass 4: Files is seeded with the lookup snapshot through
  `TaskFileDocument.reconcile`, which handles drafts and saves, instead of
  reading the file a second time.
- A zoomed Task view behaves like a tile: Files opens beside the Session.
- The remote snapshot satisfies the first automatic Files read for that
  presentation. Refresh and file watches keep using their existing reads.
- The unfocused hover monitor refreshes Ghostty's pointer state so keyboard
  input stays with the first responder. Ghostty ignores stationary pointer
  updates, so the monitor sends a leave and re-entry on press and release.
  [Pinned Ghostty cursor handling](https://github.com/ghostty-org/ghostty/blob/a60e9e2/src/apprt/embedded.zig#L997).
