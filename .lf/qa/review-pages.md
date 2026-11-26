Read when the change touches `lf page`, the page kit (`builtins/page/`), the loopback listener or Desktop's review page bridge.

# Review pages in a browser

Build the branch `lf`, then follow `server.md` for a private home: isolated
`HOME`/`LF_HOME`, inherited `LF_*` cleared, `LF_BIN` set, and `lf server run`
detached with Python (`os.fork`, `os.setsid`, `execvp`). Its `lf.log` prints
`Loopback review page listener ready port=…`. A disposable repository with
`planning: {provider: git, personal: true}`, `lf wt create qa --json` and
`lf task create --worktree <path> --title … --json` gives a Task whose page
stamps `work.kind = task`.

In the Task checkout, edit a tracked file, add an untracked one, and write
`scratch/ship-review.page.html` with an `<lf-diff>` for each, an `<lf-file>`,
a `decision` component, `data-choice` / `data-multiselect` / `data-question`
groups with `data-label`, and a `data-lf` `task.create`. `lf page build` it,
then `lf page open scratch/ship-review.html --print` prints the loopback URL.

Drive it with the `browse` MCP (Claude in Chrome may be disconnected).
`browse` refuses `file:` URLs: serve the page from `python3 -I -m http.server
--bind 127.0.0.1` on another port to reach the same clipboard host. Its
`page.evaluate` runs in an isolated world, so to see what "Copy reply" wrote,
inject a `<script>` that replaces `navigator.clipboard.writeText` and stores
the text in a DOM attribute. `browse.drag({from, to})` makes a text selection;
`page.mouse.down` is unsupported. Press `Meta+Enter` without clicking the body
first (a scrolled body click misses). `browse.emulate({colorScheme: 'dark'})`
then reload checks dark.

Read what `⌘↩` delivered from the store, read-only:
`sqlite3 "file:$LF_HOME/loopflow.db?mode=ro" "select * from steers"`. Admission
checks are plain `curl` with WebSocket upgrade headers: no `Origin`, a foreign
`Origin`, a wrong `Host` or token each give 404; the right three give 101.
Stopping the server by PID under an open page shows the restart banner and
flips the strip to `Copy reply`. Stop the server before rebuilding `lf` (the
build replaces the binary it runs), then restart it: the token changes, the
port stays (derived from the socket path), so `localStorage` drafts and
receipts survive into the reopened URL. Clear `localStorage` from the page
before a first-visit check. Dismissing Chrome's debugging bar closes the
`browse` tab; open a new one rather than reusing its id.

Write each check's page into the fragment: a rich prompt (`<p>` or `<label>`
child) beside bare `data-label` controls, and a `decision` card, whose title
must appear once. Verify the upgraded note-taking UI in both light and dark
mode:
- Drag across text spanning an inline `<code>` element → floating pill
  (`Comment (c)` · `Flag` · `Copy`) appears above the selection; `c` or
  `Comment (c)` opens the floating composer and saves a warm-underlined
  highlight across the `<code>` span; `Flag` saves immediately without typing.
- Hover a saved highlight → floating hover card offers **Edit** and **Delete**;
  the top-right `Comments (N)` button opens the slide-over sidebar listing
  notes in document order with click-to-jump pulse, inline edit, and delete.
- On `<lf-diff>` and `<lf-file>` blocks, click `+` on a line and `Shift+click`
  a second line to save a multi-line range (`<path>:<start>–<end>`); confirm
  the lines highlight, join the Comments sidebar, and appear as a `diff` chip.
- In the browser bottom dock, verify removable chips (click to jump, `×` to
  remove), the optional overall direction input (persists across reload, rides
  into the steer, clears on send and **Clear**), toggleable **Preview**, and
  that **Clear** keeps unreported `data-lf` receipts intact.

The `.lf/qa/claude-standin.py` stand-in can't answer `-p` batches, so a
headless `lf -b` that runs `lf page open` fails before reaching it;
`page_tests::loopback_serves_page_enforces_admission_and_steers_task` covers
that route.
