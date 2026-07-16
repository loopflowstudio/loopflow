# W2-202 — Separate headless PR publication from review presentation

## Problem (one screen)

Publication and presentation are welded together. `create_or_update_pr`
(`ops/pr.rs`) pushes + creates/updates the PR **and** opens the browser
(`open_url` at `pr.rs:136`, `pr.rs:155`). `finalize_remote` in `ops/land.rs:362`
opens the browser too — so `lf pr submit` and `lf pr land` pop a browser tab on
every headless run. There is no way to publish a PR without presenting it.

Cut the seam: publication becomes presentation-free; one boundary owns
presentation; only `lf pr open` presents.

## User-visible outcome

- `lf pr publish` — new headless command. Pushes, creates or refreshes the PR
  with generated/supplied title+body, prints state + URL, opens nothing.
- `lf pr open` — the one explicit review action: publishes, then presents once
  (GitHub URL in the browser, the default surface).
- `lf pr submit` / `lf pr land` — publish + finalize; present nothing.
- Agents are told to `publish` / `submit` / `land`, never `open` (unless a human
  asked for review presentation).

## Source of truth & the two operations

GitHub stays the source of truth for PR state.

- **Publication** — one owner: `create_or_update_pr` (`ops/pr.rs`). After this
  change it does push + create/update + ready + task-publication bookkeeping and
  **returns**. It never calls `open_url`.
- **Presentation** — one owner: a new `ops::present` boundary,
  `present_pr_review(url) -> OpsResult<()>`. It resolves the review surface
  (only `ReviewSurface::GithubBrowser` today; absent preference ⇒ that default)
  and shells `platform::open_url_checked`. Adding a terminal-diff / file-browser
  presenter later means adding an arm here — publication and every other caller
  stay untouched.

## Changes by surface

**Rust — publication (`ops/pr.rs`)**
- Delete the two `open_url(...)` calls (`:136`, `:155`) and the local
  `open_url` wrapper (`:660`). `create_or_update_pr` is now pure publication.

**Rust — land/submit (`ops/land.rs`)**
- Delete the `open_url(&url)` call in `finalize_remote` (`:362`) and the local
  `open_url` wrapper (`:543`). Keep the `progress.status("\n{url}\n")` line so
  the URL still prints. Submit/land present nothing.

**Rust — presentation boundary (new `ops/present.rs`)**
- `pub enum ReviewSurface { GithubBrowser }` (`#[non_exhaustive]`, room to grow).
- `pub fn present_pr_review(url: &str) -> OpsResult<()>` — resolve surface
  (default GithubBrowser), open via `platform::open_url_checked`, map opener
  failure to `OpsError` with an actionable message. Register in `ops/mod.rs`.

**Rust — CLI (`lf/mod.rs`, `lf/commands/ops/mod.rs`)**
- Add `PrCommand::Publish { model, title, body }` mirroring `Open`.
- `run_pr`: add a `Publish` arm.
- Extract `publish_current(title, body, agent, progress) -> OpsResult<PrResult>`
  wrapping `create_or_update_pr` under `with_rebase_retry` (today's `open_pr`
  body minus printing).
- `publish_pr` handler: publish, then print state + URL (reuse the `pr_status`
  format `#{n} {state} {branch}` + url; note created/updated).
- `open_pr` handler: publish, **print the URL first**, then
  `present_pr_review(&result.url)`. On presentation error, return an error whose
  message states the PR published successfully at `<url>` — the URL is already on
  stdout, so a failed browser launch fails only `pr open` and never makes the
  published PR look failed.

**Rust — help/docs**
- `builtins/LOOPFLOW.md`: add `lf pr publish` to the quick-reference and the
  `publish` vs `submit` vs `land` explainer; reframe `lf pr open` as the explicit
  review-presentation action (human-initiated), not the agent's publish verb.
- `builtins/ops/skill/pr.md`: direct agents to `lf pr publish` (produces:
  published/updated PR).
- `builtins/build/skill/task_pursue.md:36`: `lf pr open` → `lf pr publish`.
- `builtins/build/flow/ship.yaml`: `op: pr open` → `op: pr publish`.
- `docs/ops.md`, `docs/getting-started.md`, `CLAUDE.md`: document the split;
  agent-facing examples use publish/submit/land.

**Swift — Mac app**
- The app has no PR-review action today and never opens a PR URL itself (it only
  renders `.prOpened` transcript events as an icon). The proof needs the *review
  action* to delegate to `lf pr open`, not construct a URL and call
  `NSWorkspace.open`. Add a small command builder — e.g.
  `PullRequestReview.reviewCommand(worktree:) -> [String]` returning
  `["lf", "pr", "open", ...]` — and run it through the existing `lf`-shelling
  path used by other Task controls. A unit test asserts it yields `lf pr open`
  and performs no `NSWorkspace.open` on a github.com URL. Wiring a visible
  "Review" button onto the `readyForReview` Task control (RoadmapView) is thin
  polish included only if clean; the delegation helper + test is the required
  proof. Background/automation app paths use `lf pr publish`.

## End-to-end proof

Recorded presentation boundary: the fake `open` binary already stubbed in
`tests/pr_tests.rs` and `tests/land_tests.rs` becomes a **counter** (appends to a
marker file per invocation) so tests assert the number of presentation attempts.

- `lf pr publish` on a fresh branch creates the expected title/body; **0**
  presentation attempts. (`pr_tests.rs`)
- `lf pr publish` with an existing PR refreshes it; **0** presentation attempts.
- `lf pr open` after publication makes **exactly 1** presentation attempt with
  the PR URL, and only once a PR URL exists.
- `lf pr submit` and `lf pr land` each make **0** presentation attempts.
  (`land_tests.rs`)
- GitHub/push failure returns an error and makes **0** presentation attempts.
- Swift: unit test proves the review action delegates to `lf pr open` and never
  opens a URL itself.

Manual smoke (where the forked local store permits): `lf pr publish` prints
state+URL without a browser; `lf pr open` opens the browser once.

## Absent & error states

- Publish with no PR ⇒ create. Publish with a PR ⇒ refresh.
- Push/GitHub failure ⇒ actionable error, no presentation attempt.
- `pr open` presentation failure ⇒ error *after* a successful publish, PR URL
  preserved in stdout.
- Absent presentation preference ⇒ GitHub browser default.

## Exclusions

- No terminal-diff / file-browser presenter (only the boundary + GitHub default).
- No change to submit-vs-land commitment, readiness, assignment, auto-merge,
  merge gates, or title/body generation policy.
- No presentation added to any command other than `pr open`.
- The stale `lf pr open create -a` example inside `pr_message.md`'s sample body
  is illustrative copy, not a directive — left untouched.

## PR plan

One serial PR covers the Rust split + docs/skills + the Swift delegation helper
and test. If the Swift wiring grows beyond the helper+test, rotate a second
serial PR (`lf pr next`) for the Mac UI action. Runner owns rotation.
