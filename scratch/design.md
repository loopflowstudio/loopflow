# LOO-825: conversation opening

Status: attachment-aware recovery is implemented. Gate still owns affected-suite
checks, the earlier parallel host-request timeouts and isolated native-host
acceptance.

The Task brief delegates the remaining product decisions:

> Finish the remaining product details autonomously in the written design: the scope of already-open detection (same terminal versus other local hosts/Machines), repository/Wave opening and workspace command placement, and the result for a plain terminal without sibling-tab support. Do not silently change the requested mode.

## Opening contract

- `lf open` resolves one scope's primary Session, or an exact `--session`.
  Task, Wave and repository selection all open that scope's conversation.
- Direct CLI defaults to `--here`; agent calls default to `--tab`.
  `--here`, `--tab` and `--focus` are mutually exclusive. Desktop navigation
  focuses its existing view. Agent `--here` fails with a human-runnable command.
- Ordinary terminal opening reports a known existing view and offers `--focus`.
  Match Session/view identity across local hosts on the presentation Machine;
  do not enumerate other Machines. Report a known remote viewer without taking
  input. Explicit `resume` remains the way to move input.
- `--focus` creates nothing: no live view means `Not open`; an unfocusable
  view gets its location and a failure. A closed Session can reopen from history
  through ordinary `open` without replaying its prompt or moving its Graph.
- `--tab` without a supporting host fails and supplies a command to run manually.
  No implicit Desktop launch, host fallback or mode change.
- `lf workspace` owns explicit layouts, repository/Wave overviews, `--available`,
  `--no-focus` and native workspace following. Ordinary `open` ignores layouts.
  Existing standalone workflow and change inspection stay `lf task view` and
  `lf diff`. Desktop's `open --diff`, file presentation and link output remain.
- A workspace Conversation pane uses the shared Session resolver and opening
  rules. Overviews and arrivals start no agents. Already-open conversations
  retain input; requested conversations never turn into workflow panes.
- Conversation success requires attachment. Failures return to the caller,
  retain readable destination diagnostics, and allow the same command to retry
  after rechecking live attachment. Keep successful workspace tools on partial
  failure. Use existing Session/view records, not another workspace registry.

## Current approach

`select_connection` and `--start-if-free` own primary selection, closed-history
continuation and atomic non-takeover. A transient launch acknowledgement records
the transport's input-ready Return; accepting a tab or layout cannot satisfy it.
Desktop confirms its own conversation opening. Arrangements use one URL, and a
Task layout has one optional conversation acknowledgement. No workspace registry
is added. The public contract is documented in `docs/workspaces.md`.

The removed layout routing, cached starting receipts and conversation-suppression
policies need no further implementation work. Their earlier inventory is in
`b8510757c9:scratch/design.md`.

On 2026-11-10, fetching `origin/main` found no commits absent from this branch.
The base already includes #1876's Task subscription and sleep/wake reconnect
changes, and #1875's Desktop Session status changes. No separate reconnect
implementation remains in this Task.

## Recovery contract

Separate **where a view exists** from **whether its conversation attached**.
The existing contract already requires both honest attachment results and a
meaningful retry; no additional product choice is needed.

Counterexample: `lf open --task ENG-123 --host desktop` leaves a terminal surface
after attachment fails. Correcting the cause and repeating that command must
retry the conversation in that destination. A retained error surface must not
short-circuit recovery as “Already open” or “Focused existing conversation.”
At `ee5392a9a5cd66ceefe6e4f5ff361a5f53b08fe4`, `conversation_view` checked
surface existence while Desktop's opening owner separately knew failure;
`OpenStatus::Usable` combined attachment and focus. These are distinct facts, not two ways to establish
readiness. This is a source-level finding, not a reproduced native-host run.

Recovery behavior:

- Ready view: ordinary terminal opening reports its location; `--focus`
  reveals it. Neither duplicates it nor moves input.
- Pending attachment: report that it is still opening and its location. A
  timeout does not authorize another launch while that attachment remains live.
- Failed or ended attachment: ordinary opening retries through the existing
  non-replacing Session path. Desktop reuses its retained pane; native terminal
  error tabs keep their diagnostics. Saved history remains; consumed prompts
  are never replayed. Failure without provider history still explains the
  deliberate `resume` action already required by the Session owner.
- Focus only reveals an existing view, including its diagnostics; its result
  must not claim that a failed or pending conversation became usable. With no
  view it still reports `Not open` and creates nothing.

Derive view identity and attachment state from the existing Session/transport
and Desktop owners, and let the shared opening decision consume both. Replace
the presence-only `Live` decision; do not add a persisted opening lifecycle.
Keep focus results distinct from attachment results in the receipt API and its
wire fixtures. Each transient Desktop receipt has an identity: the previous
failure cannot settle a retry before LaunchServices delivers its new request.
Repository-window readiness cannot confirm Session attachment. Desktop's `.elsewhere` result must retain that meaning rather
than confirm local attachment. Workspace Conversation panes use the same facts;
successful tools and diagnostic panes survive failures. Transport input control
alone, before input-ready Return, must not stand in for readiness on retry.

The matching rule in `docs/workspaces.md`: “A view may still be opening or show
an attachment error. `--focus` only reveals it. After correcting an attachment
failure, repeat the opening command to reconnect; an attachment still in
progress is reported without launching another.” Existing agent guidance stays:
retry in the requested mode, never use `--here` inside a tool call.

Focused checks cover failure → corrected cause → same-command retry, delayed
readiness, acknowledgement timeout, and focus on ready/failed/absent views. Check
Session identity, no prompt replay, no competing input owner, retained
diagnostics and truthful receipts. Existing native-host acceptance remains.
Desktop resolution before pane creation also remains pending through its existing
opening receipt. A receipt revision distinguishes the last attempt from its retry.
No open product questions. Review page: `scratch/concept-review.html`.

## Delete — do not maintain

No deletion targets remain. Removed paths stay removed:

- Presence-only `open::Live`/`elsewhere` and surface-exists-as-live shortcuts.
  View identity and attachment now come from their existing owners.
- Focus-as-`Usable` and elsewhere-as-usable receipts. Focus confirms only focus;
  failed viewers retry in retained panes; pending attachments keep their attempt.
- Ordered host fallbacks and `RouteError::{Definite, Unknown}`. One optional
  route host replaces the list; host failures carry their diagnostic chain.
  The Swift DTO, fixtures and schema use the same shape, including null for focus.

The conversation presenter handles existing views before launch. Desktop's
receipt wait is separate from terminal presentation; both still require attachment.

## Remaining verification

Gate: affected Rust suites (`open`, `desktop`, CLI discovery, Wave resolution,
Work opening, transport and the CLI binary), documented commands, architecture,
clippy, the broader Swift suite and the Xcode terminal-fallback build. Isolated Rex/Herdr acceptance remains,
including direct opening, agent sibling and own-Session opening, same-Rex
identity, focus present/absent, closed history, plain terminals, repository/Wave
selection and failed attachment. The native arrangement fixtures alone do not
establish all of those conversation behaviors.

The earlier parallel Work-opening run had four 2s host-request timeouts; eight
serial rechecks and three workspace checks passed without explaining them.
The compression run passed the Work-opening suite in parallel without those timeouts;
isolated native-host acceptance still belongs to gate. The full earlier results are at
`ee5392a9a5cd66ceefe6e4f5ff361a5f53b08fe4:scratch/design.md`.

Focused recovery checks (inherited `LF_*` cleared, stdin closed):

- `cargo test -p loopflow --test integration work_open_tests::desktop_conversation_attachment_retry_and_focus_keep_the_retained_view` — passed: failed/ready/absent focus, same-command retry, Changes companion, pending with/without a pane, no extra launch. Socket fixture uses a short `/tmp` path after macOS canonicalization made the longer path unreachable.
- `cargo test -p loopflow --test integration -- dto_fixtures::opening_receipts_distinguish_focus_from_attachment dto_fixtures::desktop_state_retains_outer_and_inner_layouts dto_fixtures::desktop_opening_distinguishes_launch_acceptance_from_usable_and_failed work_open_tests::ordinary_open_reports_the_view_and_only_explicit_focus_reveals_it` — four passed.
- `cargo test -p loopflow --test transport_tests conversation_open -- --test-threads=1` — three passed: withheld readiness, non-takeover and closed history without prompt replay.
- `cargo test -p loopflow --lib lf::commands::desktop::tests::conversation_attachment` — two passed; linker repeated its earlier `__eh_frame` size warning.
- `cargo test -p loopflow --lib -- lf::commands::open::conversation::tests:: transport::viewer_tests::snapshot_preview_hands_off_to_a_fresh_primary_screen` — passed; acknowledgement timeout/exit and native viewer handoff covered.
- `scripts/test_desktop.sh --skip-build --filter 'conversationAttachment|composedOpeningWaitsForNativeSessionAndComparison|ServerConnectionTests/openingWaitsForNativeReadinessAndRecoversInPlace|DTOFixtureTests/desktop'` — app/tests built; eight tests passed, including eight composed-opening cases, with WindowServer denied.

Compression checks (inherited `LF_*` cleared, stdin closed):

- `cargo test -p loopflow --test integration -- work_open_tests:: dto_fixtures::work_links_carry_readable_labels_routes_and_conversation_relation dto_fixtures::opening_receipts_distinguish_focus_from_attachment` — 36 passed, two native-host tests ignored; no host-request timeouts.
- `swift build --build-tests --package-path swift` — passed; existing Swift and Ghostty linker warnings remain.
- `scripts/test_desktop.sh --skip-build --filter 'conversationAttachment|composedOpeningWaitsForNativeSessionAndComparison|ServerConnectionTests/openingWaitsForNativeReadinessAndRecoversInPlace|DTOFixtureTests/workLinksCarryReadableLabelsRoutesAndConversationRelation'` — five tests passed, including eight composed-opening cases, with WindowServer denied.
- `cargo test -p loopflow --lib -- lf::commands::open:: server::schema::tests::` — 27 passed; existing `__eh_frame` linker warning remains.
- `cargo test -p loopflow --test integration -- documented_commands:: dto_fixtures::work_links_carry_readable_labels_routes_and_conversation_relation` — five passed.
- `cargo fmt --check` and `git diff --check` — passed.

These socket/transport and headless model checks do not replace isolated
Rex/Herdr acceptance. The autonomous choices in `questions.md` need no further
product decision. Earlier file presentation returned “No Desktop Files view
attached; nothing opened.” No fallback launched.
