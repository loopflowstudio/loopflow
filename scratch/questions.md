# Open questions (LOO-1109)

Choices made with nobody present. Each is reversible; `scratch/plan.md` carries
the chosen side and `scratch/concept-review.html` frames each for review.

1. **"Open it in rex" opens the OS default browser (and never pops a window
   when headless).** Rex has no browser block (`rex block types --json`:
   onboarding, `terminal.shell`, tunnel) and `agent-browser` is not installed
   here. Passed over: launching a headed `agent-browser` from `lf page open`,
   and opening a browser from headless Graph steps. Which browser did "an
   agent-browser split" mean?
2. **`data-lf` runs immediately with an in-place receipt and stays out of the
   reply queue.** Choices (`Fix here`, `File Task`, `Drop` on a `decision`
   card) stay queued for `⌘↩`; explicit `data-lf` action buttons confirm
   inline, execute now, show their receipt in place, and only attach a compact
   `[already ran: …]` note if the person also sends a queued reply. Passed
   over: pushing `"Ran …"` into the unsent queue / Desktop `ReviewCommentsStrip`,
   and batching operations into `⌘↩`.
3. **Any `Operation` is accepted over the loopback WebSocket, no allowlist**,
   behind a per-server in-memory token, a `Host` check, and a same-origin
   `Origin` check (`page.url` stays local-Unix-socket-only, and `/ticket` is
   omitted). Follows "its basically another proxy for the lf machine server
   interface"; the earlier "1b" narrow allowlist is passed over.
4. **Task pages steer via `task.steer` immediately on `⌘↩`** (prototype 2a),
   rather than targeting the building agent's `session` (which silently drops
   steers when a headless Graph worker built the page) or typing into Rex's
   unsent terminal input (2b). Wave / standalone interactive / repo pages fall
   back to `session.steer`.
5. **The loopback token lives in memory**, so a server restart invalidates open
   pages (which show a banner and fall back to clipboard copy). Passed over: a
   token file in the lf home.
6. **Desktop owns page draft state in `ReviewComments` (no `localStorage` in
   Desktop) and keeps `ReviewRenderedView` read-only.** Avoids `SecurityError`
   on peer Machines (`about:blank`), resurrection bugs when sending from another
   file, and dual-side collisions in HTML diffs.
7. **Source is `scratch/<name>.page.html`, compiled output is
   `scratch/<name>.html`**, with `*.page.html` hidden from `scratch_pages` and
   `lf page open` refusing raw `.page.html` sources.
8. **`tour` is untouched**: it writes `scratch/tour.yaml`, not a page.
