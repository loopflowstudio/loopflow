# LOO-1109: review pages a person can answer from any host

Direction and exact words: `scratch/review-infra-notes.md`. Prototype of the
page and its three hosts: `scratch/explore.html` (round 3). Stacked on LOO-1104.

## Outcome

A review page (`ship-review`, `explore`, `concept-review`, `qa`, `demo`) is
written as a short source fragment and compiled by `lf page build`. The
compiled page is one self-contained file. On it a person drags across any
text to leave a note, clicks choices, answers questions, and presses `⌘↩`
once. Where that goes depends on where the page is open, and the text is the
same in all three:

| Host | Opens with | `⌘↩` |
| --- | --- | --- |
| Browser beside Rex or any terminal | `lf page open` → `http://127.0.0.1:<port>/<token>/f/<path>` | `session.steer` (else `task.steer`) over the page's WebSocket to `lf server` |
| Loopflow Desktop | `lf page open` → Files, as `lf desktop files` does today | notes and choices sit in `ReviewCommentsStrip` beside Diff comments; "Send to conversation" puts them in the unsent input |
| `file://`, or `lf server` stopped | double-click | copies the message |

A `data-lf` button is a different thing from the queue: it runs one typed
`Operation` now (`task.create`, `pr.land`, anything the Machine server takes),
after an inline confirm that shows the operation, and leaves a line in the
queue saying what ran.

The page's content changes too: what was built, how it works, the key data
structures and APIs, the trade-offs and invariants to know going forward. No
passing-test counts, no routine CI state, no invented terminal transcripts,
no filler choices.

## Demo

```sh
lf page build scratch/ship-review.page.html   # → scratch/ship-review.html, real hunks in it
lf page open scratch/ship-review.html         # browser opens beside the Rex pane running the agent
```

Drag across a sentence, type a note, click "File Task" on a finding, press
`⌘↩`: the agent in the Rex pane receives the steer and acts. Open the same
file in Desktop Files: the same gestures land in the comments strip next to
Diff comments.

## Findings that shape the design

- **`lf open --file` is gone.** #2034 removed it and
  `rust/loopflow/src/lf/commands/open/files.rs`; `lf desktop files PATH`
  replaced it (`rust/loopflow/src/lf/commands/desktop.rs:1788`,
  `docs/surfaces.md`). The brief's "update `lf open --file`" becomes a new
  `lf page open`, and the six skills present with it.
- **Rex has no browser pane.** `rex block types --json` lists three creators:
  onboarding, `terminal.shell`, tunnel. `agent-browser` is not installed on
  this Machine and nothing in the repository names it. "Open it in rex" is
  therefore the OS default browser beside Rex. Made for review; see
  `scratch/questions.md` 1.
- **The server already speaks its protocol over WebSocket.**
  `rust/loopflow/src/server/remote.rs` wraps a `WebSocketStream` as
  `Messages<S>: Carrier` and hands it to `service::welcome` /
  `service::session` / `service::ticketed`. Its handshake refuses any request
  with an `Origin` header (`remote.rs:361`). The loopback carrier is the same
  code with the opposite admission rule, not a new protocol.
- **A token on the loopback is the whole user.** `session.start` runs a
  caller's `argv`; `task.file` reads checkouts. Nothing is gained by an
  allowlist in front of that, which matches "its basically another proxy for
  the lf machine server interface". What needs defending is the browser:
  other origins can open WebSockets to `127.0.0.1` and CORS does not apply to
  them. Hence the three checks under Transport.
- **`hello.client.kind` is a free string** (`machine/wire.rs:93`), so a page
  names itself `page` with no wire change.
- **Desktop's page view can take a script handler without touching the
  offline rule.** `TaskHTMLView` builds `WebPage.Configuration()` and adds the
  `WKContentRuleList` to `userContentController`
  (`swift/LoopflowMac/Views/TaskHTMLView.swift:128-137`); a script message is
  not a network load. It loads an HTML string, so a page read from a peer
  Machine gets the bridge too.
- **Desktop comments are line-anchored.** `ReviewComment` holds a
  `ReviewSelection` (path, source, first, last) and
  `ReviewComment.message` formats `reference + words`
  (`swift/LoopflowMac/ReviewDiff.swift:748-790`). `ReviewSelection.Source` has
  no case for a rendered page, and `reconcile` assumes numbered lines. A page
  note is anchored to a section and quoted text, so the comment's anchor
  becomes one of two kinds.
- **The message is one line.** `addToConversation` ends in
  `insertText` (`GhosttyTerminalView.swift:311`), which refuses any control
  character, newlines included; `ReviewComment.message` already collapses
  whitespace and numbers items `(1) … (2) …`. The page's format follows it.
- **Desktop sends typed writes only.** `RegistryQuery.write` is private and
  takes a `ServedWrite` case (`Loopflow/Services/RegistryQuery.swift:323`,
  `:791`); nothing in the Mac app steers. A page's operation arrives as JSON,
  so `ServerClient` gains one send that forwards a JSON operation as the
  request's `operation` and lets the server validate it.
- **Nothing in the app registers a script handler today**, and `WebPage`
  tests need `.requiresDisplay` (`LOOPFLOW_NATIVE_TESTS=1`). The handler's
  receiving function is plain Swift, tested headless without a page.
- **No JavaScript runs in CI.** No node, no browser. The runtime stays small,
  the compiler (Rust) does what can be done ahead of time, and the runtime is
  exercised in `qa` in a real browser and in Desktop.
- **`tour` writes `scratch/tour.yaml`, not a page.** It shares nothing with
  the kit and is left alone; `ship-review` keeps writing the tour.

## Design

### Transport: `server/loopback.rs`

A third listener in `lf server` beside the Unix socket and the Tailscale
carrier: TCP `127.0.0.1:0`, plain HTTP. The token is 32 random bytes made at
server start and held in memory.

| Route | Answer |
| --- | --- |
| `GET /<token>/f/<absolute path>` | the file, typed by extension. HTML carries `Referrer-Policy: no-referrer`, `Cache-Control: no-store` and a CSP of `default-src 'self' 'unsafe-inline' data: blob:; connect-src 'self'`, so a served page reaches nothing but this server, as Desktop's rule list already ensures there |
| `GET /<token>/machine` (upgrade) | the Machine protocol, one text frame per message, `Admission::Local` |
| `GET /<token>/ticket` (upgrade) | a bulk message, as `/v1/ticket` |

Every request must carry `Host: 127.0.0.1:<port>` (DNS rebinding); an upgrade
must carry `Origin: http://127.0.0.1:<port>`; the token compares in constant
time. Anything else is 404 with no body. There are no CORS headers and no
`OPTIONS`.

`remote.rs::connection` is generalized over its handshake check and reused;
`Messages<S>` moves beside `Carrier`. GET is served by `hyper` (already in the
lock file through axum) with the upgrade passed to
`WebSocketStream::from_raw_socket`.

The CLI learns the URL from the server over the Unix socket: a
`ServerOperation` `page.url {path}` → `{url}`. The token is never printed by
`lf server status`. A server restart changes port and token; an open page
then shows "lf server restarted; run `lf page open <path>`" and its `⌘↩`
copies.

Passed over: a token file in the lf home so URLs survive a restart (a secret
on disk for little gain), and Task-relative URLs (pages outside a Task, in a
repository or Wave conversation, would need a second form).

### `lf page`

```sh
lf page build <source> [--base <rev>]   # scratch/x.page.html → scratch/x.html
lf page open <page> [--print]           # Desktop when the conversation is viewed there, else the browser
```

**Source** is body HTML only: no `<html>`, `<style>` or `<script>`. The
compiler wraps it with the theme and the runtime and expands:

- `<lf-diff path="…" lines="142-150">` with `<lf-note line="145">` children →
  the real hunk from `git diff <base>` (default base: the Task's, else the
  merge base with the default branch), each note a callout on its line under
  a "Hide line notes" toggle (`data-notes`). A path or range the diff does not
  contain is an error.
- `<lf-file path="…" lines="…">` → the lines as they stand, for code that did
  not change.
- `<lf-component name="…">` → `.lf/components/<name>/template.html` (plus its
  `style.css`), over the builtin of that name. Builtins: `decision`
  (fix here / its own Task / drop), `question`, `concepts` (type or table or
  API, shape and location, ownership and invariant), `before-after`.
- `data-choice`, `data-multiselect`, `data-question`, `data-lf` pass through
  for the runtime.

**Theme** is `.lf/theme.css` when the repository has one, else the builtin.
This repository gets one from `DESIGN.md`'s tokens. A theme sets custom
properties only; light and dark both.

**Checks**, each an error naming the element and line: a repository path that
does not exist, a 7–40 hex SHA that `git cat-file -e` cannot find, a
`data-lf` value that is not a known `Operation` (validated with the type
behind `lf server describe`), a leftover `<script>` or `<style>`.

**Stamp**: `<script type="application/json" id="lf-page">` with `task`,
`session` (the building agent's), `directory`, `base`, `head`, `path`. All
three hosts read it; nothing is asked of the server to learn where a page
belongs.

Kit assets (runtime JS, base CSS, builtin components) live in
`rust/loopflow-definitions/src/builtins/page/` and are embedded like skills.
`*.page.html` is left out of `scratch_pages` so Desktop lists the compiled
page once.

### Runtime

One script, inlined by the build. No page writes JavaScript.

- **Notes**: a selection inside the page offers "Note"; the note keeps the
  nearest heading and the quoted text, and the quote stays highlighted
  (CSS Highlight API, as `TaskHTMLView`'s find already uses).
- **Controls**: `data-choice` (one of a group), `data-multiselect`,
  `data-question` (free text). Each is one queued line.
- **`data-lf`**: click → inline confirm showing kind and fields → the
  operation runs → its result or error shows in place and "Ran `task.create`
  → LOO-1200" joins the queue. On `file://` there is nothing to run it, so it
  queues "Requested: …" for the agent.
- **Strip**: fixed at the bottom while anything is queued: count, Clear, and
  one button named for the host (Steer agent / Copy reply). `⌘↩` presses it.
- **Drafts** persist in `localStorage` under the page's path and `head`, so a
  regenerated page starts clean and a reload does not.
- **One message format** for all hosts, written once in the runtime, on one
  line: `(1) \`scratch/ship-review.html\` § <section> › "<quote>" <words> (2) …`.
  The runtime builds each item's reference; Desktop receives it ready-made
  and only joins it with its Diff comments.

Host is chosen once at load: `window.webkit.messageHandlers.loopflow` →
Desktop; an `http:` origin whose `/machine` socket welcomes → loopback; else
clipboard.

### Desktop bridge

`TaskHTMLView` registers a reply-capable script handler named `loopflow`.

- `items`: the page's whole queue, replacing what Desktop held for that page.
  They join `ReviewComments.items`; `ReviewComment`'s anchor becomes lines or
  page (the reference the runtime wrote), and `ReviewComment.message` joins
  both. Page items skip line reconciling. The
  page's own strip is hidden in Desktop: `ReviewCommentsStrip` is the one
  strip, count and "Send to conversation ⌘↩" unchanged. After a send Desktop
  tells the page its queue is spent.
- `operation`: the page's JSON operation, forwarded by `ServerClient` as it
  is and answered to the page with the server's result or error.

Both call sites get it (`TaskFilesView.swift:545`,
`ReviewRenderedView.swift:82`). The offline rule list, link handling, find
and reload-in-place are untouched. A page without the runtime behaves as
today.

### Skills

`explore`, `concept-review`, `qa`, `demo` and `ship-review` each say what
their page must show and stop describing how to build HTML. The shared part
(write `scratch/<name>.page.html`, the elements, `lf page build`,
`lf page open`, and the writing rules: real code and output over commentary,
numbered questions so "2b" answers, proposed marked apart from existing,
nothing that reports routine checks) is one section printed by
`lf help page`, which each skill points to, as `ship-review` already points
to `lf help tour`. A repository's `extends: lf:<skill>` adds its own
components and rules on top.

`ship-review`'s page order becomes: what was built; how it works, on real
hunks; the key data structures and APIs; trade-offs and invariants going
forward; findings as `decision` components; questions. "How to try it" stays
as one line with the command.

## Acceptance check at gate

Headless, one Rust integration test in `rust/loopflow/tests/` with a
`MachineServer`, a `TestRepo` Task and a held work session:

1. `lf page build scratch/ship-review.page.html` on a fixture fragment → the
   output contains the fixture's real `+` lines, the runtime, the stamp; a
   fragment naming a missing path exits non-zero naming it.
2. `lf page open scratch/ship-review.html --print` → a `127.0.0.1` URL; GET
   returns the page with the CSP header.
3. Upgrade without `Origin`, with a foreign `Origin`, with a wrong `Host`,
   with a wrong token → 404 each.
4. Upgrade with the right three → `hello`/`welcome`, then `task.steer` →
   the steer is in the Task's Session history.

Swift, headless: an `items` message given to the handler's receiving function
yields comments in `store.comments`; a Diff comment and a page note send
through `ReviewCommentsStrip` as one line with no control character.

`qa` owns what CI cannot run: the runtime in Chrome against the branch
`lf server`, and in the Dev app.

## Absent and error states

| State | What the person sees |
| --- | --- |
| `lf server` not running at `lf page open` | the command starts it as every client does; `--print` likewise |
| Server restarted under an open page | banner with the `lf page open` command; `⌘↩` copies |
| Steer target gone (Session ended, Task done) | the server's error sentence in the strip; the queue stays |
| `data-lf` operation fails | its error in place; nothing joins the queue |
| Desktop, no conversation open | the strip's existing disabled state and help text |
| Page built without the kit (old or hand-written) | renders as today, no strip |
| Peer-owned Task | Desktop works (routes by Task). `lf page open` on another Machine prints the owning Machine's loopback URL, unreachable from here: excluded |

## Operational boundary

- Loopback connections count against the server's 64-connection limit and
  the five-second first-message rule; a page holds one connection and closes
  it on `pagehide`.
- A served file is read whole, capped at the existing bulk limit.
- `lf page build` runs `git diff` once per source file path, not per element.

## Exclusions

- A Rex web pane, remote Rex hosts (`rex forward`) and phone access to pages.
- Typing into the agent's unsent terminal input from the browser (the
  prototype's 2b); the loopback host steers.
- Live subscriptions on a page (a diff that updates as the agent works). The
  socket allows it; no builtin component uses it yet.
- `measure`'s `scratch/measure.html` table and `tour`.

## What goes

- The HTML-crafting paragraphs in `explore.md`, `concept-review.md`,
  `qa.md`, `demo.md`, `ship-review.md` ("One self-contained file, readable in
  light and dark…", "borrow its palette, type and layout"), replaced by the
  pointer to `lf help page`; the visual-language sentence added in
  2b6b03ad5e lives on as `.lf/theme.css`.
- `lf desktop files scratch/<page>.html` in those five skills and the page
  example in `docs/surfaces.md` → `lf page open`. `lf desktop files` itself
  stays for files and line ranges.
- `remote.rs`'s private `connection` and `Messages` in their one-carrier form.
- Survives: every existing `scratch/*.html` keeps opening; Diff comments,
  their re-anchoring and `⌘↩`; the offline rule; `lf help tour`.

## Passes

1. **Loopback carrier and one answering page.** `loopback.rs`, `page.url`,
   `lf page open`, a hand-wrapped page with the minimum runtime (queue, strip,
   steer). Shows whether a browser page can drive the Machine protocol through
   `service::session` unchanged and whether the three admission checks hold.
   Gate steps 2–4 pass. This is the pass that could prove the plan wrong.
2. **`lf page build` and the full runtime.** Elements, components, theme,
   checks, stamp; notes by highlight, controls, `data-lf`, drafts.
   `.lf/theme.css` for this repository. Gate step 1.
3. **Desktop bridge.** Handler, the two-kind anchor, one strip. Swift tests.
4. **Skills and docs.** The five skills, `lf help page`, `docs/surfaces.md`,
   `server/README.md`, goldens. This Task's own `ship-review` is the first
   page built with the kit.

## Done when

- A page built by `lf page build` and opened by `lf page open` from a Rex
  pane takes a highlighted note and a choice, and `⌘↩` reaches the agent.
- The same file in Desktop puts the same items in the comments strip beside
  Diff comments and sends them as one message.
- The same file double-clicked copies that message.
- A `data-lf` button files a Task from the page.
- The five builtin skills produce their pages through the kit, and a
  repository skill with `extends: lf:ship-review` and a `.lf/components/`
  override changes the page without restating the base.
