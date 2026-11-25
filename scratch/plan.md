# LOO-1109: review pages a person can answer from any host

Direction and exact words: `scratch/review-infra-notes.md`. Prototype of the
page and its three hosts: `scratch/explore.html` (round 3). Concept review:
`scratch/concept-review.html`. Stacked on LOO-1104.

## Outcome

A review page (`ship-review`, `explore`, `concept-review`, `qa`, `demo`) is
written as a short body fragment (`scratch/<name>.page.html`) and compiled by
`lf page build` into one self-contained file (`scratch/<name>.html`). On it a
person drags across any text to leave a note, clicks choices, answers
questions, and presses `⌘↩` once. Where that goes depends on where the page is
open, and the message format is the same in all three:

| Host | Opens with | `⌘↩` |
| --- | --- | --- |
| Browser beside Rex or any terminal | `lf page open` → `http://127.0.0.1:<port>/<token>/f/<path>` | `task.steer` (for Task pages) or `session.steer` (Wave / interactive Session / repo) over WebSocket to `lf server`; strip shows the server's `delivery.sentence` |
| Loopflow Desktop | `lf page open` → Desktop Files (`lf desktop files`), quietly | Swift `ReviewComments` owns the page's draft state; notes, choices and answers sit in `ReviewCommentsStrip` beside Diff comments; "Send to conversation" puts them in the unsent input |
| Headless Graph step (auto / `-b`) | `lf page open` prints the URL and never pops a browser window | Desktop's `scratch_pages` watcher shows a quiet `Updated` notice if open; person opens the URL or file when ready |
| `file://`, or `lf server` stopped | double-click | copies the one-line message |

Direct actions (`data-lf`) are distinct from the reply queue. A `data-lf`
button runs one typed Machine `Operation` now (`task.create`, `pr.land`, etc.)
after an inline confirm showing its kind and fields, then turns into an
in-place receipt (`Filed LOO-1200`). It never enters the unsent reply queue or
Desktop's `ReviewCommentsStrip`; if the person also sends a queued reply via
`⌘↩`, a short `[already ran: …]` suffix rides along so the agent does not
duplicate the work.

The page's content changes too: what was built, how it works on real diffs, the
key data structures and APIs, and the trade-offs and invariants to know going
forward. No passing-test counts, no routine CI state, no invented terminal
transcripts, no filler choices.

## Demo

```sh
lf page build scratch/ship-review.page.html   # → scratch/ship-review.html, real hunks in it
lf page open scratch/ship-review.html         # browser opens beside Rex, or Files opens in Desktop
```

Drag across a sentence, type a note, pick "File Task" on a finding (`decision`
choice), press `⌘↩`: `task.steer` delivers the single-line message to the Task
and the strip prints the server's `delivery.sentence` while the agent in Rex
acts. Click an explicit `data-lf` action button (e.g. "File Task now") and
confirm: the Task is created immediately and the button shows its new ID in
place without popping the reply strip. Open the same file in Desktop Files: the
same notes and choices land in `ReviewCommentsStrip` beside Diff comments,
survive switching to a code file, and stay cleared after sending.

## Findings that shape the design

- **`lf open --file` is gone.** #2034 removed it and
  `rust/loopflow/src/lf/commands/open/files.rs`; `lf desktop files PATH`
  replaced it (`rust/loopflow/src/lf/commands/desktop.rs:1788`,
  `docs/surfaces.md`). `lf page open` handles review pages across hosts and
  calls `lf desktop files` when the conversation is viewed in Desktop.
- **Rex has no browser pane.** `rex block types --json` lists onboarding,
  `terminal.shell` and tunnel. `agent-browser` is not installed on this Machine
  and nothing in the repository names it. Opening beside Rex therefore uses the
  OS default browser. Made for review; see `scratch/questions.md` 1.
- **`session.steer` silently black-holes steers sent to a finished worker.**
  `engine/session.rs:34-46` saves a steer on any existing Session without error,
  even a finished Graph worker nothing will ever resume. Because headless Graph
  steps (`explore`, `concept-review`, `qa`, `ship-review`) exit right after
  building their page, stamping and steering the building `session` would lose
  reviews. `task.steer` (`write.rs:1482`, `engine/session.rs:91`) resolves the
  Task's active work or starts its primary interactive Session, forwards to peer
  Machines automatically, and returns `delivery.sentence`.
- **Wire `Operation` types have `deny_unknown_fields` and zero defaults.**
  `task.create` and `pr.land` each require 10 keys including `directory` and
  (for `task.create`) `request_id`. Authors cannot reliably hand-write complete
  wire JSON in HTML attributes; `lf page build` expands partial JSON into
  strict, validated `Operation` JSON using the build context, and the runtime
  generates a fresh UUID `request_id` per confirmation.
- **The server already speaks its protocol over WebSocket.**
  `rust/loopflow/src/server/remote.rs` wraps a `WebSocketStream` as
  `Messages<S>: Carrier` and hands it to `service::welcome` / `service::session`.
  Its handshake refuses any request with an `Origin` header (`remote.rs:361`).
  The loopback carrier reuses `Messages<S>` and `service::session` with the
  opposite admission rule (`Host` + same-origin `Origin` + constant-time token).
  No `/ticket` route is added until a page consumer needs bulk or terminal
  bridging.
- **A token on the loopback is the whole user.** `session.start` runs a
  caller's `argv`; `task.file` reads checkouts. Nothing is gained by an
  allowlist in front of that ("its basically another proxy for the lf machine
  server interface"). What needs defending is the browser (foreign origins can
  open WebSockets to `127.0.0.1` without CORS) and remote peers (`page.url`
  must be local-Unix-socket-only, like `mobile.status` / `mobile.configure`).
- **`hello.client.kind` is a free string** (`machine/wire.rs:93`), so a page
  names itself `page` with no wire change.
- **Loopflow already owns diffing with untracked files and renames.**
  `ops::task::snapshot::diff_snapshot` and `task_diff_base` handle untracked
  files (`git diff --no-index`) and renames; raw `git diff <base>` misses both.
- **Desktop `TaskHTMLView` runs offline and loads peer pages on `about:blank`.**
  `TaskHTMLView` adds `WKContentRuleList` to `userContentController`
  (`swift/LoopflowMac/Views/TaskHTMLView.swift:128-137`); a script message is
  not a network load. When `store.machine != nil`, `baseURL` is `nil` →
  `about:blank`, where `window.localStorage` throws `SecurityError`. Furthermore,
  switching to a source file unmounts `TaskHTMLView`, so sending comments while
  unmounted would leave `localStorage` dirty and resurrect sent notes on return.
  Therefore Swift (`ReviewComments`) is the sole owner of page draft state in
  Desktop; `localStorage` is never touched under the Desktop bridge.
- **`ReviewRenderedView` renders two copies of the same HTML file (old vs. new
  in Diff).** Wiring the interactive bridge there would collide both sides on
  the same path and expose live `data-lf` mutations on historical commits. Only
  `TaskFileReadingView` (`TaskFilesView.swift:545`) attaches the bridge;
  `ReviewRenderedView` stays read-only.
- **Desktop comments are line-anchored and one line.** `ReviewComment` holds a
  `ReviewSelection` (path, source, first, last) and `ReviewComment.message`
  collapses whitespace onto one line (`GhosttyTerminalView.swift:311` refuses
  control characters including newlines). `ReviewComment`'s anchor becomes
  lines or page (with the reference the runtime wrote); page items skip line
  reconciling.
- **Draft identity must survive reloads without surviving page rewrites on the
  same `HEAD`.** `explore` round 2 rewrites `scratch/explore.html` without a
  new commit. Stamping a `build_id` (short hash of the compiled page body)
  keeps drafts across reloads and Desktop tab switches while resetting cleanly
  on any rebuild.
- **Desktop sends typed writes only.** `ServerClient` (`ServerClient.swift`)
  sends `ServerOperation` cases; adding one raw-JSON `Operation` send lets a
  page's validated `data-lf` payload execute through the existing connection,
  substituting Desktop's local repository directory when a peer-Machine page
  carries a remote path.
- **No JavaScript runs in CI.** The runtime stays small, Rust does all ahead-of-time
  expansion and validation, and `qa` exercises the runtime in Chrome and Dev
  Desktop.
- **`tour` writes `scratch/tour.yaml`, not a page.** Left untouched;
  `ship-review` keeps writing it.

## Design

### Transport: `server/loopback.rs`

A third listener in `lf server` beside the Unix socket and the Tailscale
carrier: TCP `127.0.0.1:0`, plain HTTP. A 32-byte random token is generated at
server start and held in memory.

| Route | Answer |
| --- | --- |
| `GET /<token>/f/<absolute path>` | the file, typed by extension, capped at `MAX_BULK`. Sets `Referrer-Policy: no-referrer`, `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`. HTML also carries `Content-Security-Policy: default-src 'self' 'unsafe-inline' data: blob:; connect-src 'self'`, matching Desktop's offline rule |
| `GET /<token>/machine` (upgrade) | the Machine protocol, one text frame per message, `Admission::Local`, `remote_control: None`, `loopback: None` |

Every request must carry `Host: 127.0.0.1:<port>` (DNS rebinding); upgrades
must also carry `Origin: http://127.0.0.1:<port>`; the token compares in
constant time. Anything else returns 404 with no body. No CORS headers, no
`OPTIONS`, no `/<token>/ticket`.

`Messages<S>` moves beside `Carrier` in `service.rs` (or a shared module) and
both `remote.rs` and `loopback.rs` reuse it. HTTP GET and upgrade run on
`hyper` (already in the lock file), handing upgraded streams to
`WebSocketStream::from_raw_socket`.

The CLI asks the server for a page's URL over the local Unix socket via a
local-only `ServerOperation` `page.url { path }` → `{ url }` (refused on remote
Tailscale and loopback connections, matching `mobile.status` /
`mobile.configure`). The token is never printed by `lf server status`. A server
stop or restart changes port/token or closes the socket; an open page then shows
a banner `Loopflow server disconnected — run lf page open <path>` (or the
server's handshake error) and its `⌘↩` copies.

Passed over: a token file on disk (a secret on disk for little gain), a
`/<token>/ticket` route (no page caller), and Task-relative URLs (wave/repo
pages have no Task, and `/<token>/f/<absolute path>` lets relative assets like
`qa.html` screenshots resolve naturally).

### `lf page`

```sh
lf page build <source> [--base <rev>]                           # scratch/x.page.html → scratch/x.html
lf page open <page> [--host auto|desktop|browser] [--print]     # open where you work, or print URL
```

#### `lf page open` routing (`--host auto`)

1. **Direct CLI call** (`agent_caller().is_none()`): if `LF_DESKTOP` is set,
   present in Desktop Files (`lf desktop files <page>`); otherwise open the
   loopback URL in the OS default browser and print it.
2. **Interactive agent call** (`agent_caller()` present, session is interactive
   — `session.interactive == true` in SQLite, or a live `viewed_placement()`):
   if `viewed_placement()` is `OpenHost::Desktop` (or `LF_DESKTOP` is set),
   present via `lf desktop files <page>`; otherwise (Rex, cmux, tmux, herdr,
   or a plain terminal like Ghostty/iTerm/Terminal.app) open the loopback URL
   in the browser and print it.
3. **Headless agent call** (`agent_caller()` present, `session.interactive ==
   false` — or session missing from the store with no `viewed_placement()` —
   e.g. background Graph step or `lf -b`): **never pop a browser window**;
   print the loopback URL and exit 0. Desktop's `scratch_pages` watcher
   surfaces the quiet `Updated` notice if the Task is open there.
4. **`--host desktop` / `--host browser`**: explicit override; fails cleanly
   without falling back to another host.
5. **`--print`**: prints the loopback URL without opening anything (starts
   `lf server` if needed).
6. Passing a `*.page.html` source path fails with the exact `lf page build`
   command rather than serving raw custom tags.

#### `lf page build`

**Source** is body HTML only: no `<html>`, `<head>`, `<style>`, or `<script>`.
The compiler wraps it with the theme CSS, base CSS, runtime JS, and JSON stamp,
and expands:

- `<lf-diff path="…" [lines="142-150"]>` with optional `<lf-note line="145">`
  (new-side line) and `<lf-note old-line="140">` (deleted line) children →
  real hunks from Loopflow's diff machinery (`ops::task::snapshot::diff_snapshot`
  + `task_diff_base` when in a Task, falling back to `git diff` + `--no-index`
  for untracked files so newly created uncommitted files and renames work).
  Omitting `lines` renders all hunks for `path`. Notes sit under a "Hide line
  notes" toggle (`data-notes`). A path or line range not in the diff is an
  error naming element and line.
- `<lf-file path="…" [lines="1-40"]>` with optional `<lf-note line="…">` →
  numbered lines as they stand on disk, for code that did not change.
- `<lf-component name="…">` → `.lf/components/<name>/template.html` (plus
  optional `style.css`), overriding the builtin of that name. Substitution is
  intentionally tiny: HTML-escaped `{{attr}}` from the element's attributes and
  `<slot>` / `<slot name="…">` for child HTML. Zero loops, zero conditionals.
  Builtins:
  - `decision` (`id`, `title`, `recommendation`, body `<slot>`, renders
    `data-choice` buttons for Fix here / File Task / Drop)
  - `before-after` (named slots `before` and `after`)
  Plain HTML `<table>` and `<div data-choice="…">` stay plain HTML instead of
  forced component wrappers.
- `data-choice`, `data-multiselect`, `data-question` pass through unchanged.
- `data-lf='<partial JSON>'`: expanded into a complete, strict wire `Operation`
  (injecting `directory` from the build checkout and `null`/`false` defaults
  for omitted nullable/boolean fields from the operation schema, keeping a
  sentinel for per-click `request_id` on `task.create` / `session.new`) and
  validated by deserializing as `Operation`. Unknown fields, unknown `kind`, or
  missing required fields fail the build naming element and line.

**Theme** is `.lf/theme.css` when the repository has one, else the builtin.
This repository gets one from `DESIGN.md`'s tokens. Themes set CSS custom
properties only, covering light and dark.

**Checks**, each a build error naming element and line:
- missing repository path in `<lf-diff>`, `<lf-file>`, or `[data-path]`;
- unknown 7–40 hex SHA in `<code>` (containing at least one digit and one hex
  letter, skipping `data-no-check`) verified with `git cat-file -e`;
- invalid `data-lf` operation;
- leftover `<script>` or `<style>` in source.

**Stamp**: `<script type="application/json" id="lf-page">` containing:
- `work`: `{ "kind": "task", "issue": "…" }` | `{ "kind": "wave", "wave": "…" }`
  | `{ "kind": "session", "session": "…" }` (only when the building caller is
  an interactive standalone Session, never a headless worker/batch) |
  `{ "kind": "repository" }`
- `directory`, `base`, `head`, `path`, `build_id` (short SHA-256 prefix of the
  compiled body HTML), `protocol`: `{ "revision": <wire::REVISION>, "min_revision": <wire::MIN_REVISION> }`
  so the runtime speaks the server's compiled revision without hardcoding a
  magic number.

Kit assets (runtime JS, base CSS, builtin components) live in
`rust/loopflow-definitions/src/builtins/page/` and are embedded like skills.
`is_scratch_page` excludes `*.page.html` so Desktop lists only the compiled
page.

### Runtime

One small script, inlined by `lf page build`. No page writes JavaScript.

- **Notes**: dragging across text offers a "Note" popover; saving keeps the
  nearest heading and quoted text and highlights the quote with the CSS
  Highlight API (`::highlight(lf-note)`).
- **Controls**: `data-choice` (single select per group), `data-multiselect`,
  `data-question` (free text). Each contributes one item to the reply queue.
  `data-label` names the item in the formatted reply; when a control has no
  direct prose/heading child (`<p>`, `<label>`, `<h3>`, `<h4>`, `<legend>`),
  `base.css` renders `data-label` as the prompt above the inputs; when it does
  have one (such as a fuller numbered question `<p>`), CSS suppresses the
  `::before` pseudo-element and gives the `<p>` full row width
  (`flex-basis: 100%`) so the prompt is never duplicated and stays selectable
  for highlight-to-comment.
- **Reply strip**: fixed at the bottom while the reply queue is non-empty
  (hidden in Desktop, where `ReviewCommentsStrip` is the one strip): count,
  Clear, and one action button named for the host (`Steer <target> ⌘↩` in
  loopback browser, `Copy reply ⌘↩` offline). `⌘↩` triggers it.
  - On loopback submit: sends `task.steer` when `work.kind == "task"`, else
    `session.steer` scoped to `wave` / `session` / repository. Shows the
    server's `delivery.sentence` (or error) in the strip; clears the queue and
    marks receipts `reported` only on success.
  - Clicking **Clear** drops unsent notes, choices, answers, and offline
    actions without marking unreported `data-lf` receipts as `reported`, so
    `[already ran: …]` still rides along on the next reply.
- **Direct actions (`data-lf`)**: separate from the reply queue. Click → inline
  confirm showing `kind` and non-null fields → runtime injects a fresh
  `crypto.randomUUID()` `request_id` when required (reused on retry of the same
  confirm) and sends the `Operation` → button becomes an in-place receipt
  (`Filed LOO-1200` or inline error). Receipts never trigger the reply strip and
  never enter Desktop's `ReviewCommentsStrip`. If the person later submits a
  non-empty reply queue via `⌘↩`, the runtime appends
  `[already ran: task.create → LOO-1200]` to that message. Offline (`file://`),
  clicking a `data-lf` button cannot execute, so it notes `"Offline — added to
  reply"` and queues `Run: …` for clipboard copy.
- **Draft & receipt persistence**:
  - In browser / `file://`: `localStorage` keyed by `path + build_id` (guarded
    with `try/catch`). Reload preserves work; rebuild starts clean.
  - In Desktop: **never touches `localStorage`**. Swift owns state via the
    bridge below.
- **One message format** across all hosts, on one line:
  `(1) \`scratch/ship-review.html\` § <section> › "<quote>" <words> (2) …`.
  The runtime formats each item's reference; Desktop joins page items with Diff
  comments using the same one-line formatter.

Host detection runs once at load: `window.webkit?.messageHandlers?.loopflow` →
Desktop; `http:` origin with `/<token>/f/…` whose `/<token>/machine` WebSocket
completes `hello` (using `stamp.protocol`) / `welcome` → loopback; else
clipboard. On a loopback URL, if the WebSocket fails during handshake (e.g.
un-ID'd `error` such as protocol mismatch, or stale token 404) or closes after
`welcome` (server stopped or restarted), the runtime shows a truthful banner
(the server's error message, or `Loopflow server disconnected — run lf page
open <path>`) and falls back to `Copy reply ⌘↩`.

### Desktop bridge

`TaskFileReadingView` (`TaskFilesView.swift:545`) registers a reply-capable
`WKScriptMessageHandler` named `loopflow` on `TaskHTMLView`.
`ReviewRenderedView` (`ReviewRenderedView.swift:82`, Diff old vs. new HTML)
leaves the handler unset so historical/diff HTML stays read-only.

Messages from the page to Swift:
- `{ "type": "ready", "path": "…", "build_id": "…" }` → Swift replies with the
  saved opaque `state` for `(path, build_id)` (or `null` if none or spent).
  Survives switching away to code files and back, works on peer Machines
  (`baseURL == nil` / `about:blank`), and resets cleanly on rebuild or send.
- `{ "type": "sync", "path": "…", "build_id": "…", "state": {…}, "items": […] }`
  → Swift stores `state` under `(path, build_id)` and replaces that page's
  entries in `ReviewComments.items`. `ReviewComment` gets a two-case anchor
  (`.lines(ReviewSelection)` | `.page(path: String, reference: String)`),
  skipping line reconciliation for `.page`. `ReviewCommentsStrip` counts and
  sends both together on one line. When `ReviewComments.insert(into:)` succeeds
  (even while another file is open and `TaskHTMLView` is unmounted), Swift
  clears all stored page `state` entries and notifies the active `WebPage` if
  mounted.
- `{ "type": "operation", "operation": {…} }` → Swift forwards the strict JSON
  `Operation` through a new raw-operation send on `ServerClient` (substituting
  the local repository directory when viewing a peer-Machine checkout whose
  stamped path is remote) and replies to the page with the server's JSON result
  or error.

The offline `WKContentRuleList`, link handling, find, and reload-in-place are
untouched. A hand-written page without the runtime behaves identically to today.

### Skills

`explore`, `concept-review`, `qa`, `demo`, and `ship-review` each say what
their page must show and stop describing how to hand-craft HTML/CSS/JS. The
shared authoring contract (`scratch/<name>.page.html`, `<lf-diff>`, `<lf-file>`,
`<lf-component>`, `data-choice`/`data-multiselect`/`data-question`/`data-lf`,
`lf page build`, `lf page open`, and the writing rules: real code and output
over commentary, numbered questions so "2b" answers, proposed marked apart from
existing, no passing test counts or routine CI checks) lives once in
`lf help page`, which each skill points to (as `ship-review` already points to
`lf help tour`). A repository's `extends: lf:<skill>` adds its own components
and rules on top without restating the base.

`ship-review`'s page order becomes: what was built; how it works on real hunks;
key data structures and APIs; trade-offs and invariants going forward; findings
as `decision` components; numbered questions. "How to try it" stays as one line
with the command. `LOOPFLOW.md` updates its presentation line to mention
`lf page open` for review pages alongside `lf desktop files` for source files.

## Acceptance check at gate

Headless Rust integration test in `rust/loopflow/tests/` with a `MachineServer`,
a `TestRepo` Task, and a held work session:

1. `lf page build scratch/ship-review.page.html` on a fixture fragment (including
   an untracked file `<lf-diff>`, a `<lf-note old-line="…">`, a `decision`
   component, and a partial `data-lf` `task.create`) → output contains real `+`
   lines, strict validated `data-lf` JSON, runtime, and `build_id` + `work.task`
   stamp. Missing path, unknown SHA, or invalid `data-lf` exits non-zero naming
   element and line.
2. `lf page open scratch/ship-review.html --print` → `127.0.0.1` URL; GET
   returns HTML with CSP, `Referrer-Policy: no-referrer`, and
   `Cache-Control: no-store`. Passing `.page.html` exits non-zero with the build
   command. Calling `page.url` from a non-local carrier is refused.
3. Upgrade without `Origin`, with a foreign `Origin`, with a wrong `Host`, or
   with a wrong token → 404 each.
4. Upgrade with the right three → `hello`/`welcome` (`client.kind = "page"`),
   then `task.steer` → steer is saved on the Task and returns `delivery.sentence`.
   Headless `lf page open` (with `agent_caller` and no viewer) prints the URL
   without invoking the OS browser opener.

Swift, headless:
- `ready` → `sync` → send (`insert(into:)`) → `ready` again returns `nil` state
  (proving no resurrection after unmounted send).
- Diff line comment + page note send through `ReviewCommentsStrip` as one line
  with no control characters; `reconcile` skips `.page` comments without crash.

`qa` owns what CI cannot run: Chrome against branch `lf server`, and Dev
Desktop (including switching files before sending and clicking a `data-lf`
button).

## Absent and error states

| State | What the person sees |
| --- | --- |
| `lf server` not running at `lf page open` | command starts it as every client does; `--print` likewise |
| Server stopped or restarted under an open page (or stale URL / protocol mismatch) | banner with server error or `Loopflow server disconnected — run lf page open <path>`; `⌘↩` copies |
| Steer target finished or unavailable | server's `delivery.sentence` or error in the strip; queue stays on error |
| `data-lf` operation fails | inline error on the button; retryable with same `request_id` |
| Desktop, no conversation open | `ReviewCommentsStrip`'s existing disabled state and help text |
| Page built without the kit (hand-written) | renders as today, no strip |
| Peer-owned Task | Desktop works (Swift owns drafts — no `about:blank` `localStorage` error; `ServerClient` forwards operations). Browser loopback `lf page open` on another Machine is excluded |

## Operational boundary

- Loopback connections count against the server's 64-connection limit and the
  5-second first-message rule; a page holds one connection and closes on
  `pagehide`.
- Served files are read whole, capped at `MAX_BULK`.
- `lf page build` computes diffs once per file path, not per `<lf-diff>` tag.

## Exclusions

- A Rex web pane, remote Rex hosts (`rex forward`), and phone access to pages.
- Typing into the agent's unsent terminal input from the browser (prototype 2b);
  the loopback host steers via `task.steer` / `session.steer`.
- Live subscriptions on a page (diffs updating as the agent works). The socket
  supports it; no builtin component uses it yet.
- `/<token>/ticket` bulk/terminal bridge over loopback.
- `measure`'s `scratch/measure.html` table and `tour`.

## What goes

- HTML/CSS/JS crafting paragraphs in `explore.md`, `concept-review.md`,
  `qa.md`, `demo.md`, `ship-review.md`, replaced by the pointer to
  `lf help page`; the visual-language sentence from `2b6b03ad5e` lives on as
  `.lf/theme.css`.
- `lf desktop files scratch/<page>.html` in those five skills and the page
  example in `docs/surfaces.md` → `lf page open`. `lf desktop files` stays for
  source files and line ranges.
- `remote.rs`'s private `Messages` in its one-carrier form (moved beside
  `Carrier`).
- Survives: every existing `scratch/*.html` keeps opening; Diff comments, their
  re-anchoring, and `⌘↩`; the offline `WKContentRuleList`; `ReviewRenderedView`
  side-by-side HTML diffs; `lf help tour`.

## Passes

1. **Done — Loopback carrier and one answering page.** Built `server/loopback.rs`
   (`127.0.0.1:0`, 32-byte random in-memory token, `Host` + constant-time token +
   same-origin `Origin` checks, CSP / `Referrer-Policy` / `Cache-Control` /
   `X-Content-Type-Options`, `404` empty body on refusal), moved shared
   `Messages<S>` into `server/service.rs`, added local-Unix-socket-only
   `ServerOperation::PageUrl` (`page.url`) via `LocalAuthority` (refused on both
   Tailscale and loopback), regenerated `tests/fixtures/dto/server_schema.json`,
   added `lf page open` (`--host auto|desktop|browser`, `--print`, headless
   quiet URL print without invoking OS browser opener, `.page.html` refusal),
   and embedded the page kit assets in `rust/loopflow-definitions/src/builtins/page/`.
   Gate steps 2–4 pass in `rust/loopflow/tests/page_tests.rs`.
2. **Done — `lf page build` and the full runtime.** Built `lf/commands/page_build.rs`
   expanding `<lf-component>` (`.lf/components/<name>/` overriding builtins
   `decision` and `before-after`), `<lf-diff>` (reusing `ops::task::checkout_file_diff`
   once per file path for rename + untracked support, optional `lines`,
   `<lf-note line|old-line>` with toggle), `<lf-file>`, partial→strict `data-lf`
   expansion against `server::schema::describe()` + `Operation` deserialization
   (`__LF_REQUEST_ID__` sentinel replaced per click with `crypto.randomUUID()`),
   forbidden tag / missing path / `git cat-file -e` SHA checks naming element and
   line, readable `work` stamp + 16-char `build_id`, `.lf/theme.css` from
   `DESIGN.md`, `is_scratch_page` excluding `*.page.html`, and the full runtime
   (`runtime.js`, `base.css`, `theme.css`). Gate step 1 passes in
   `rust/loopflow/tests/page_tests.rs`.
   *What this slice taught:* `task.create` returns `result.task.identifier` (not
   top-level `identifier`), and `context::explain_in` returns the internal
   `TaskId` rather than the readable identifier (`LOO-1109`) — both fixed so the
   runtime button (`Steer <issue> ⌘↩`) and `Filed <id>` receipt show readable IDs.
3. **Done — Desktop bridge.** Added `TaskHTMLBridge` (reply-capable
   `WKScriptMessageHandlerWithReply` named `loopflow`, registered in `.page`
   content world) on `TaskHTMLView` and attached it only in `TaskFileReadingView`
   (`TaskFilesView.swift`), leaving `ReviewRenderedView` read-only (`bridge == nil`).
   Updated `ReviewComment` with a two-case `Anchor` (`.lines(ReviewSelection)` |
   `.page(path: String, reference: String)`), skipped line reconciliation for
   `.page`, collapsed control characters and whitespace onto one line (with any
   `[already ran: …]` receipt suffix), and made `ReviewComments` the sole owner
   of page draft state (`pageReady`, `syncPage`, clearing `pageStates` and
   bumping `pageClearGeneration` on successful `insert(into:)` so mounted
   `TaskHTMLView` calls `window.__lfDesktopApplyState?.(null)` in `.page`).
   Added `ServerLink.sendRawOperation` / `ServerClient.rawOperation` without a
   new `ServerOperation` case, wired peer-Machine `directory` substitution via
   `TaskFilesStore.localRepository`, and aligned `TaskFilesStore.isScratchPage`
   to exclude `*.page.html`. Headless Swift tests pass (`ReviewDiffTests`,
   `ServerClientTests/rawOperationForwardsJSONPayloadWithRequestId`,
   `ServerProtocolTests/everyEncodedOperationIsTheDescribedOperation`,
   `TaskFilesTests/reviewPageBridgeOwnsStateClearsAfterSendAndForwardsOperations`).
   *What this slice taught:* `runtime.js` already exposed `window.__lfDesktopApplyState`
   from Pass 2 (no JS/Rust edit needed); encoding `RawOperationRequest` directly
   in `ServerLink.sendRawOperation` reuses `admitted` busy retries and upload
   tickets without adding a `ServerOperation` case or touching `ServerProtocolTests`;
   and `TaskFilesStore.isScratchPage` also needed the `*.page.html` filter so local
   FSEvents match Rust's `ops/task/find.rs`.
   Open shape questions for compress: `page_build.rs` is 1434 lines in one
   file; `TaskFilesTests/reviewPageBridgeOwnsStateClearsAfterSendAndForwardsOperations`
   checks eight behaviors in one test and could split along its numbered steps.
   Deferred to qa: the runtime against `__lfDesktopApplyState` in Dev Desktop
   (switch files before sending, click a `data-lf` button); no JS runs in CI.
4. **Done — Skills and docs (`929e95f0a6`).** Rewrote `explore`, `concept-review`, `qa`,
   `demo`, and `ship-review` under `rust/loopflow-definitions/src/builtins/task/skill/`
   to point at `lf help page`, compile `scratch/<name>.page.html` with
   `lf page build`, and present `scratch/<name>.html` with `lf page open`
   (replacing hand-written HTML/CSS/visual-language paragraphs and
   `lf desktop files`). Expanded `lf help page` (`after_help` in `lf/mod.rs`)
   with exact control markup and build checks, updated `LOOPFLOW.md` to name
   `lf page open` beside `lf desktop files`, added the loopback carrier and
   `page.url` to `rust/loopflow/src/server/README.md`, mapped `lf page` in
   `docs/dev/reference.md` (fixing `check_architecture.py` 27/27), regenerated
   the six prompt goldens under `tests/goldens/`, verified all six pre-written
   user docs (`surfaces.md`, `orchestration.md`, `getting-started.md`,
   `server.md`, `security.md`, `index.md`) match the shipped CLI and runtime,
   and built LOO-1109's own `scratch/ship-review.page.html` →
   `scratch/ship-review.html` with the kit.
   *What this slice taught:* `scripts/check_architecture.py` enforces top-level
   CLI family coverage in `docs/dev/reference.md`, so adding `lf page` required
   mapping it under the `Client API server` row there. Also, `builtins::tests`
   now enforces the kit contract across all five review skills and `LOOPFLOW.md`.
   Realign check: `cargo test -p loopflow-definitions --lib builtins::` 13/13 pass.

5. **Done — Post-build concept-review alignment.** Fixed all four
   conceptual/lifecycle mismatches from `scratch/concept-review.html`:
   - **`lf page open` interactivity routing (`rust/loopflow/src/lf/commands/page.rs`):**
     added shared `load_session` (reused by `page/build.rs`) to read
     `session.interactive` from SQLite. `session.interactive == false` always
     routes to `PrintQuiet` (even if `LF_DESKTOP` is inherited);
     `session.interactive == true` routes to `Desktop` when viewed in Desktop
     or `LF_DESKTOP` is set, else `Browser` (fixing Ghostty/iTerm/Terminal.app
     interactive sessions); missing session rows fall back to
     `viewed_placement()`. Added unit tests in `page.rs` and an integration
     test (`page_open_routes_interactive_plain_terminal_to_browser_and_headless_worker_quietly`)
     in `rust/loopflow/tests/page_tests.rs`.
   - **Control prompt vs. reply label (`rust/loopflow-definitions/src/builtins/page/base.css`,
     `rust/loopflow/src/lf/mod.rs`):** guarded
     `[data-choice|data-multiselect|data-question][data-label]::before` with
     `:not(:has(> p, > label, > h3, > h4, > legend))` and gave
     `:is([data-choice], [data-multiselect]) > :is(p, label, h3, h4, legend)`
     `flex-basis: 100%; margin: 10px 0 2px;`. Documented both bare and rich
     prompt forms in `lf help page`.
   - **Loopback protocol single source of truth & truthful disconnect banner
     (`rust/loopflow/src/lf/commands/page/build.rs`,
     `rust/loopflow-definitions/src/builtins/page/runtime.js`,
     `docs/server.md`, `docs/surfaces.md`):** stamped
     `protocol: { revision, min_revision }` from `crate::server::wire` into
     `PageStamp`, removed hardcoded `10` from `runtime.js`, captured un-ID'd
     handshake `error` frames, and surfaced a truthful banner (handshake error
     or `Loopflow server disconnected — run lf page open <path>`) on any
     non-unload close of a loopback WebSocket while falling back to
     `Copy reply ⌘↩`.
   - **Separate `Clear` from `Send` on receipts
     (`rust/loopflow-definitions/src/builtins/page/runtime.js`):**
     parameterized `clearDrafts({ markReported })` so **Clear** passes
     `markReported: false` (preserving `[already ran: …]` for the next reply)
     while `submitReply` success and Desktop `__lfDesktopApplyState(null)` pass
     `markReported: true`.
   - Rebuilt `scratch/ship-review.html` and `scratch/concept-review.html` with
     the branch `lf` binary.

### Remaining (later Graph steps)

- **qa (Chrome pass 2 + Dev Desktop):** verify Pass 5 fixes in Chrome (plain
  `data-label` control + rich `<p>` question without duplicate heading, `Clear`
  preserving `[already ran: …]`, truthful disconnect/stale-token banner) and
  Dev Desktop live if reachable.
- Open questions 1–8 in `scratch/questions.md` stay for review (notably which
  browser "an agent-browser split" meant).

## User docs written ahead of implementation

- `docs/surfaces.md` (`lf desktop files` narrowed to source files; new `## Review pages` section covering `lf page open`, host routing, highlight/choice/`⌘↩` replies, `data-lf` direct actions, reloads/restarts, peer Machines, and `lf page` reference link)
- `docs/orchestration.md` (updated `ship-review` description; new `### Build and customize review pages` section covering `scratch/<name>.page.html`, `lf page build`, `<lf-diff>`, `<lf-file>`, `<lf-component>`, `data-*`, build checks, `.lf/theme.css`, `.lf/components/`, and `extends: lf:<skill>`)
- `docs/getting-started.md` (updated `## 4. Pick up the conversation` for comprehension-focused `scratch/ship-review.html` and `lf page open`)
- `docs/server.md` (added `## Browser review pages` for the loopback listener/token lifecycle and updated the local-only network claim)
- `docs/security.md` (documented loopback listener admission, CSP, `page.url` local-only boundary, and URL credential handling under `## Local client server`)
- `docs/index.md` (added `.lf/components/` and `.lf/theme.css` to `## Where files live`)

## Done when

- A page built by `lf page build` and opened by `lf page open` from Rex takes a
  highlighted note and a finding choice, and `⌘↩` delivers a `task.steer` that
  reaches the agent even if a headless Graph worker built the page.
- A `data-lf` button confirms inline, executes `task.create`, and shows its new
  Task ID in place without polluting the reply queue or Desktop's comments strip.
- The same compiled file in Desktop puts notes and choices in
  `ReviewCommentsStrip` beside Diff comments, preserves them across file
  switches (including peer Machines), clears them cleanly after sending, and
  leaves `ReviewRenderedView` diffs read-only.
- The same compiled file double-clicked on `file://` copies the exact same
  one-line message.
- Headless `lf page open` prints the URL and never pops a browser window.
- The five builtin skills build their pages through the kit, and a repository
  skill with `extends: lf:ship-review` plus `.lf/components/` overrides changes
  the page without restating the base.
