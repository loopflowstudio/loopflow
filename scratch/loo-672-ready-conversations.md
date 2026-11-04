# LOO-672 — Ready Task conversations

Accepted design, reconciled November 4, 2026 against the working diff on
`a32c385044`, merged infrastructure through `ac8e088f5f`, and fresh LOO-717/498
Task status. Jack Heart approved the revised November 4 design: “ok, proceed.”
Implementation and review publication are authorized through interactive demo;
shipping, merging and installation are not. The pre-infrastructure plan is
superseded and preserved at
`a32c385044:scratch/research/loo-672-pre-infrastructure-plan.md`.

## Accepted outcome and constraints

Task conversations open without an Open click after launch, selection and restart.
Recent unfinished Tasks stay warm for fast input; returning lands at the bottom.
Jack selected that memory tradeoff on November 4. Completed Tasks release on
departure through LOO-717. Close removes warm intent; refresh cannot undo Close,
but a deliberate later visit may reopen the conversation.

Jack selected takeover without Fork on November 4, preserving the last-screen
and explicit **Take back** interaction. Selection and polling never transfer
another client's input. The overlay explains the holder using Rust's labels and
prepared command; without a saved screen it shows the explanation alone. The
independent CLI fork remains available on the same Task and checkout.

Viewing, prefetch and restore never start a Graph, submit captured input, create
a provider thread or replay interrupted input. Explicit Task launch retains its
authority; navigation observes the resulting conversation. Missing native history
requires explicit Start/New. Missing checkout, account or Machine evidence stays
an explicit action or failure, without checkout reconstruction or local fallback.

The selected Task opens first after relaunch, followed by the bounded saved warm
set where safe. Saved layout and status are display evidence, not retention or
readiness. Cold return paints existing history while reconnecting the surviving
owner or resuming the same provider thread; connecting and failure remain visible.

The initial engineering limits are eight recent unfinished Task conversations
across repository windows and two concurrent preparations. Visible panes remain
retained, with exceptions reported. These are provisional values, not a measured
memory budget or numbers chosen by Jack. No NSView is shared between windows.
Eviction releases Desktop's viewer and hold; protected work and other hosts may
keep the provider running. Shells retain their independent lifecycle.

## Current implementation

- Rust's existing Session selection and preparation now accept typed reveal
  intent. `human_session::reveal` checks native history, current attachment and
  location under the launch lock, avoids `open_waiting`, and carries a
  non-replacing claim into transport. A competing viewer receives no automatic
  takeover. Explicit resume retains its existing behavior.
- `SessionsWorkspaceRegistry` and `RepositoryWorkspaces` maintain the bounded
  recent set, visible exceptions, persisted recent/closed intent and preparation
  concurrency. Session `launchId` invalidates late preparations after release.
  Completed departure, eviction and Close reuse LOO-717 retention.
- `WorkModel.revealPrimaryConversation` now includes Tasks; `SessionsStore.select`
  uses reveal intent. Hidden warm Session panes remain mounted. Linked Task entry
  and cache restoration share that intent.
- The last available screen excerpt backs the elsewhere overlay; Rust supplies
  Take back and holder text. Cold history reuses the existing VT preview/relay,
  with no second durable screen format. Live relay native-client attachment
  announces readiness separately from surface creation and saved frames. This
  signal has not yet established visibly usable Claude/Codex input.
- Strict Rust/Swift fixtures include reveal actions and opening intent. The old
  detached-only gate, Task exclusion, unconditional unfinished departure release
  and blank elsewhere offer are removed. No provider quota, second idle timer,
  reboot sweep, Fork button or snapshot decoder was added.

## November 4 implementation reconciliation

Linked Task entry now prepares and adopts reveal intent end to end. Explicit
Start/New and Take back keep their existing authority. Workspace request identity
is invalidated on Close, eviction and quit, including Close before a Session ID
arrives. Late results cannot install panes or publish opening errors after release.
Prepared results are adopted once; they do not cause a second preparation.
The registry queues preparations separately from `revealConversation`, which
shares one fresh location read between restored retention and preparation. The
transport's `record_relay_terminal` records association only after the input claim.

WorkCache restores the saved destination. Saved readings cannot schedule reveal;
fresh planning and Session reads trigger it through their existing model owners.
The selected workspace is queued first. Restored background Tasks validate their
recorded Machine/checkout before reporting retention; fresh completion removes
finished Tasks from the warm set. Explicit Close survives restart. Drafts and
existing native surfaces continue through their existing owners.

LOO-717 parent commit `76750bdc55` was integrated with `lf sync --manual`, preserving
PR #1811 as stack parent. Its single lifecycle owner and existing Task migration
now supply confirmed-host-exit recovery, per-process Desktop occurrences and
optional retaining-owner evidence. Parent design/coordination details are preserved
at that commit. No competing migration or cleanup executor was introduced.

## Delete — do not maintain

- Removed automatic resume/adopt-open in `prepareLinkedTask` /
  `connectLinkedOpening`; explicit Start/New and Take back remain.
- Removed selection-discarding `WorkModel.restore` behavior and its nil-selection
  expectations in `WorkCacheTests`; Machine-change invalidation remains.
- Removed cached-reading authorization of background retention/preparation.
  Saved layout, drafts and history remain display evidence.
- Removed the duplicate restore-location preflight in `revealConversations`;
  `revealConversation` checks location and cancellation before adoption.
- Parent integration removed stable `desktop:<Machine>:<Task>` occurrences and
  their persisted wall-clock revision keys. Per-process owner evidence now uses
  LOO-717's sole retirement path and migration.

The three reconciled implementation gaps are closed. Close/eviction also settle
the opening receipt as failed, so a discarded late result leaves no permanent
opening indicator. Focused checks passed; gate and interactive demo retain the
boundaries below.

## Infrastructure and proof boundaries

708/#1802, 709/#1804, 710/#1801, 712/#1800 and 720/#1809 are merged. They supply
thin viewers, protected idle retirement, persisted VT history, separate appearance
and input ownership, and current provider status. Codex/OpenCode retain launchers;
VT replay has bounded history and does not repaint images. Reuse those owners.

LOO-717 owns eligible two-minute idle-agent retirement and fifteen-minute-total
completed-and-away shell/checkout retirement, cancellation on return, protected
work and other-host retention. Eligible uncommitted files may be discarded under
its existing safeguards; LOO-672 adds no deletion policy. LOO-498/#1814 remains
open and idle with delivery gaps; its latest coordination comment records the
717 owner-evidence API. Rex still owns native adapter/layout/job settlement.

720's candidate Claude/Rex/Desktop demonstration and CI passed, without proving
machine-wide installation or live Codex/OpenCode behavior. 719's installed macOS
idle-preservation proof was unstarted at the design review; this pass did not
refresh it. No installed or cross-host acceptance follows from these foundations.

Gate retains affected Rust/Swift suites, Clippy, formatting and Linux isolation.
Coverage must include real Task navigation, cached-to-fresh restart, strict DTOs,
Session CLI, non-replacing attach races, status across reconnect and eviction,
protected drafts/requests, Close, and completed departure.

Demo retains real Desktop with Claude and Codex: A→B→A with immediate input and
unchanged provider/editor/draft identity; cold return with history; takeover from
a named Rex adapter; relaunch and stale-owner/reboot recovery in a disposable
environment. Measure first paint separately from usable input and Desktop + lf +
provider private footprint with twenty visited Tasks. Record protected owners and
visible-pane exceptions, then state the measured budget. Eight remains provisional.
The rendered interaction and these measurements require demo/review, not a headless
substitute. No unresolved product decision remains before that evidence.

Product GOAL and full MEMORY were read; no immediate child memory directories
exist in this checkout. The Wave records durable direction separately from this plan.

Checks (November 4): `scripts/test_desktop.sh --jobs 4 --filter 'WorkDestinationTests|WorkCacheTests|WorkspaceRetentionTests|DTOFixtureTests.taskHostOwner'` passed 93 tests; after the opening-receipt correction, `--filter WorkDestinationTests.lateSidebarRevealIsDiscarded` passed both Close/eviction cases. After simplification, `scripts/test_desktop.sh --jobs 4 --filter 'WorkCacheTests|WorkspaceSessionLifecycleTests|WorkDestinationTests.lateSidebarRevealIsDiscarded'` passed 24 tests and `cargo test -p loopflow --test transport_tests prepared_reveal_preserves_a_later_controlling_viewer` passed. The unchanged `cargo test -p loopflow --lib reveal_requires_history_and_never_publishes_a_first_turn` and `cargo test -p loopflow --lib retirement_confirmed_host_exit_finishes_without_desktop_and_still_waits_for_shell_jobs` each passed one test; `cargo fmt --all --check`, `git diff --check` and `lf context` passed. Gate owns broader suites/Clippy; configured/rendered usability and memory proof remain with demo.
