# LOO-672 — Ready Task conversations

Accepted design, reconciled November 4, 2026 against the working implementation
above `7bc74405fa`, merged infrastructure through `ac8e088f5f`, and fresh LOO-717/498
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

Jack Heart added on November 4: “make sure to prefetch working tasks on load”
(source: `loo-672-load-prefetch.md`). Fresh repository state discovers idle, active
and moving Tasks, including those absent from saved recent history. The selected
Task leads, then saved recent working Tasks, then other working Tasks by their
shared update time; other saved unfinished Tasks fill remaining space. Prefetch
uses the existing reveal queue and location validation, preserves Close, and
never waits before rendering. A Task with no existing conversation stays
unavailable; prefetch never creates one.

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

Source inspection at `7bc74405fa` confirms the three earlier implementation gaps
are closed: linked preparation/adoption uses reveal, WorkCache preserves selection
behind fresh-read validation, and Desktop reports per-instance owner evidence.
Machine-change invalidation remains. Close/eviction also settle
the opening receipt as failed, so a discarded late result leaves no permanent
opening indicator. Focused checks passed; gate and interactive demo retain the
boundaries below.

## Concept review — November 4

Outcome of the follow-up concept review: nothing to change in the design and no
open product choice. Recovery and the later load-prefetch requirement are
implemented, with focused headless passes recorded above. This
follows Jack Heart's accepted requirement that missing history needs explicit
Start/New; it does not authorize a new thread through navigation.

The review traced normal return, missing-history recovery, competing ownership
and cached-to-fresh repository load through the shared actions and reveal claim.
Keep the existing usage guidance and lifecycle owners. No next implement direction
is added; the next proof remains gate coverage and the native demo below. This
review inspected source and fixtures without rerunning runtime checks or claiming
native acceptance.

Normal return, takeover, Close and restart retain the accepted model above:
one durable Session, one input owner, bounded Desktop warmth, and LOO-717's
release owner. No additional lifecycle or persistence owner is needed.

The counterexample at `7bc74405fa` was a preparation-only **Start conversation**
label: ordinary Session refresh replaced it with **Resume**, while the opening
refusal survived as terminal `.ended` output. Preparation-only tests missed the
refresh. The review illustration preserves that dated finding; it is not a
description of the repaired working tree.

The shared Rust Session projection now supplies Start, Take back and reveal
availability to preparation, ordinary listings and resume explanation. It reads
the current input's recorded resume reference,
including imported history before attachment. Native-history, account, checkout
and non-replacing claim validation remain at launch. No persisted recovery owner
or migration was added.

Desktop renders the supplied action label and help without English-label
conditions. A refused reveal has transient `.unavailable` state, distinct from
terminal `.ended` output; fresh actions can replace it with Take back or clear it
when saved history arrives. A transient failure retains Retry, which prepares
reveal rather than explicit resume. The README explains the explicit Start rule.

Saved and newly discovered Tasks share `prefetch` admission; the separate
registry `restore` path is removed. Saved non-working Tasks fill spare capacity
without evicting working Tasks. Ordering resolves each Task key and saved rank
once; location validation and LOO-717 retention remain unchanged.

Focused proof covers reveal → ordinary read → Start with no attachment or input
publication; current holder and imported-history changes; rendered Start and
Take back; and non-replacing Retry. The fresh-load fixture discovers twelve working Tasks
without saved visits, warms eight with two preparations, excludes other Task
states, preserves saved Close and leaves selection unchanged while loading
completes. Restart fixtures retain selected-first preparation and fresh-read
validation. Native/provider demonstration remains with
demo. The review illustration is historical evidence, not a native demonstration.

## Infrastructure and proof boundaries

708/#1802, 709/#1804, 710/#1801, 712/#1800 and 720/#1809 are merged. They supply
thin viewers, protected idle retirement, persisted VT history, separate appearance
and input ownership, and current provider status. Codex/OpenCode retain launchers;
VT replay has bounded history and does not repaint images. Reuse those owners.

LOO-717/#1811 is published and stands at demo; its latest integrated base is
`b81df60579`. It owns eligible two-minute idle-agent retirement and fifteen-minute-total
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

Checks (November 4): `git diff --check` and `lf context` — passed after documentation reconciliation; no runtime edits or test reruns. Reused `scripts/test_desktop.sh --jobs 4 --filter 'WorkspaceSessionLifecycleTests|WorkDestinationTests|WorkCacheTests|WorkspaceRetentionTests'` — build and 105 tests passed headless with inherited `LF_*` unset and stdin closed; Rust recovery retains its recorded 18-test pass, plus DTO and entry/restore/owner-evidence passes. Gate owns affected suites, Clippy/formatting and Linux; demo owns native usability and memory. Earlier evidence: `7bc74405fa:scratch/loo-672-ready-conversations.md`.


## Review publication evidence — November 4

`lf sync --manual` completed without conflicts against the accepted LOO-717
parent `b81df60579`; the planned strategy is now noop. Reviewed the entire parent
range and uncommitted recovery/prefetch work. Publication remains for review,
with interactive demo before any landing decision.

`uv run python scripts/test.py --base b81df60579 --reuse-passing --rust-filter
'test(ops::human_session::) | test(store::sqlite::sessions::) | test(transport::) |
test(lf::commands::write::) | test(session_cli_tests::) | test(agent_startup_tests::) |
test(transport_tests::) | test(served_reads::) | test(dto_fixtures::) | binary(lf) |
binary(session_lifecycle_tests)'` passed 218 Rust tests, inventory, formatting,
CLI build and Clippy. The initial architecture check exposed three inherited
LOO-717 tables missing from the map; documented their existing owners in the
architecture map and table inventory, then `scripts/check_architecture.py` passed.

The headless Desktop build passed. The broad Swift run was stopped by the runner
for host security pressure: syspolicyd held 225% CPU for three samples above the
200% limit. It was not a product timeout or test assertion; that run is incomplete.
The previous 105-test focused pass remains recorded above. Full Swift and broader
transport coverage remain for gate/CI; Linux isolation remains with capable CI.
`check_swift_multiplatform_boundaries.py`, `cargo fmt` and `git diff --check` passed.

The initial filter did not select standalone served_reads or transport_tests.
After checking host pressure, ran the served-read contract (1 passed, 106.38s,
including its isolated CLI fixture build) and
`transport_tests prepared_reveal_preserves_a_later_controlling_viewer` (1 passed,
5.55s), both through `scripts/test_network.py` with inherited LF variables unset
and stdin closed. Total publication Rust evidence: 220 passing tests. Logs are in
`.lf/tmp/gate/run-56483/`, `/tmp/loo-672-served-reads.log` and
`/tmp/loo-672-reveal-race.log`. No native usability or memory claim follows.
