# LOO-672 — Ready Task conversations

Accepted design, reconciled November 4, 2026 against the screenshot and launch-intent
repairs in the working tree above `df07fd9b5f`, recorded upstream through
`dfd454cfc7`, and fresh LOO-717/498 Task status. Jack Heart approved the revised November 4 design: “ok, proceed.”
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

## Implementation owners

Rust’s shared Session projection supplies Start, Take back and reveal availability
from current-input history and ownership on every read. `human_session::open_without_replacing`
rechecks history, location, account and ownership under the launch lock. Transport
claims do not replace another viewer; preparation starts no turn. Absent Sessions
get a direct Start action from resume explanation, without fabricating Session state.

`SessionsWorkspaceRegistry` owns warm intent, admission and the reveal queue;
`RepositoryWorkspaces` bounds it across windows. `WorkModel` orders fresh-load
prefetch separately from reconciling panes. Saved and fresh Tasks share admission.
A fresh location read supplies both background retention and preparation. Close,
eviction and quit fence late results, including requests without a Session ID;
linked openings carry explicit Start or passive reveal intent through adoption.

Desktop distinguishes unavailable opening from terminal completion. Refresh can
replace Start with Take back or clear the refusal when history arrives. Retry
remains passive. Hidden warm panes stay mounted; cold return uses existing VT
history. The live relay readiness event is separate from saved frames and surface
creation; it has not yet proved usable native Claude/Codex input.

The integrated LOO-717 parent `b81df60579` owns lifecycle, host-exit recovery and
per-process retention evidence. LOO-672 adds no migration, retirement executor,
provider quota, idle timer, reboot sweep or snapshot decoder. Architecture and
earlier implementation/review evidence: `df07fd9b5f:scratch/loo-672-ready-conversations.md`.
That source preserves the earlier concept review’s mistaken “nothing to change”
conclusion and the preparation-only recovery label lost on refresh; the screenshot
correction below supersedes that conclusion.

## Infrastructure and proof boundaries

### Completed replacement (November 4 correction)

Completed cuts; no predecessor remains to maintain:

- `TaskConversationButton` replaces unconditional Open in empty/collapsed panes
  with the shared explicit Start offer when no Session exists.
- `LinkedSession.preparedIntent` replaces fixed reveal intent in linked adoption;
  navigation/Retry remain reveal, and explicit Start is non-replacing.
- `SessionsWorkspace.reportPresence` selects the local query and Task together;
  only remote locations use remote routing and observed remote Task identity.
- Resume explanation returns only Start for an absent Session; no unavailable
  Reveal action or fabricated Session state is needed there.

Recovery, Close, location and non-replacing-claim tests remain applicable.
The launch-intent conflation identified below is also replaced; the screenshot
corrections themselves have no predecessor left to maintain.

The correction reuses Rust's resume explanation for an absent Session and its
existing Session actions for missing native history. A failed explanation stays
unknown. Actual launch re-reads Session ownership; Start permits first-thread creation
without replacing input, and elsewhere input requires Take back. The headless proof uses a disposable
initialized store and real CLI, including the production Start button.

### Demo findings, November 4 — correction implemented

The subsequent interactive demo supersedes the concept-review conclusion that
no further implementation was needed. Jack Heart's screenshot shows a Task with
no Session stuck at passive Open/Retry, plus a local-Machine cleanup error.
Source and CLI reproduction confirm two gaps: provide an explicit Start when
the Task has no Session (distinct from an existing Session missing native history),
and send local retention to the local lifecycle owner rather than an unresolved
remote-Machine lookup. Preserve passive navigation and remote semantics.
The correction now uses the shared resume explanation for no-Session Start,
carries explicit intent through linked preparation/adoption, and routes local
presence locally. Headless real-CLI tests cover the production button after
refresh and retain/departure/return. A separate candidate and reproducible setup
are in [loo-672-demo.md](loo-672-demo.md). The accepted product choices are
unchanged. The launch-intent repair below is also implemented; publication
verification is recorded below, and the native demonstration remains.

708/#1802, 709/#1804, 710/#1801, 712/#1800 and 720/#1809 are merged. They supply
thin viewers, protected idle retirement, persisted VT history, separate appearance
and input ownership, and current provider status. Codex/OpenCode retain launchers;
VT replay has bounded history and does not repaint images. Reuse those owners.

LOO-717/#1811 is published and stands at demo; its latest integrated base is
`b81df60579`. It owns eligible two-minute idle-agent retirement and fifteen-minute-total
completed-and-away shell/checkout retirement, cancellation on return, protected
work and other-host retention. Eligible uncommitted files may be discarded under
its existing safeguards; LOO-672 adds no deletion policy. Fresh November 4 status
shows LOO-498/#1814 open and moving through corrections, with its current step
running compress. Rex still owns native adapter/layout/job settlement; that activity
does not establish native acceptance or change LOO-672's lifecycle owner.

Recorded upstream `dfd454cfc7` (#1815, LOO-723) now pairs Desktop and CLI updates
and preserves running helpers during installation. That change is not integrated
here and does not repair this branch candidate's main-store schema mismatch.
The disposable candidate already pairs its app and CLI; installation remains
outside the accepted review boundary. No duplicate updater belongs in LOO-672.

720's candidate Claude/Rex/Desktop demonstration and CI passed, without proving
machine-wide installation or live Codex/OpenCode behavior. 719's installed macOS
idle-preservation proof was unstarted at the design review; this pass did not
refresh it. No installed or cross-host acceptance follows from these foundations.

## Start cannot become Take back — repaired November 4

The headless concept review found a launch race after the screenshot correction:
Start prepared ordinary `session resume`, so a client acquiring input before
execution could be replaced. A fresh ownership read alone could not fix that:
resume authorizes takeover of the newly observed holder. This was a source-derived
counterexample, not a reproduced native failure. Jack Heart's accepted explicit
Take back decision already resolves the behavior; no new product choice was needed.

The current implementation preserves three intents through Desktop preparation,
served writes, returned argv, execution and the transport claim:

- Reveal requires existing history and never replaces another viewer.
- Start carries `--start-if-free`, permitting a first thread without replacement.
- Take back uses ordinary CLI resume, retaining its explicit takeover semantics.

`human_session::open_without_replacing` replaces the former reveal operation and
shares `open_waiting_locked` with CLI resume. Ownership and first-thread admission
use the existing launch lock; reveal/Start attach with a non-replacing relay claim.
Transport delegation re-executes the same intent. No second launcher, persisted
opening lifecycle, migration or Desktop-only preflight was added. Provider switching
and independent CLI fork retain their explicit semantics.

`SessionsStore` now carries `SessionOpenIntent` directly through preparation and
adoption, removing the action-to-intent round trip and the `.open`/`.moveHere`
collapse. Prepared argv survives refresh; refusal adopts current holder evidence
and offers Take back. Passive Retry can reveal existing history after the holder
leaves. Load prefetch, Close fences, warm bounds and LOO-717 retention are unchanged.
The Start behavior is documented in `swift/README.md`.

Inspected source and saved test logs cover execution of the actual prepared command,
a later controlling viewer, and a competing claim inserted between the ownership
read and Attach. The winning viewer retains input; refusal starts no extra provider
thread or turn. Normal Start and explicit Take back keep one Session/capture for
both an absent Session and an existing Session without history.

The earlier regression incorrectly passed an env-wrapped command's arguments to
`lf`, allowing a parsing error to resemble ownership refusal. The surviving fixture
executes `open_argv[0]` and all remaining arguments, checks holder continuity and
uses the existing PTY helper for first-thread contention. The earlier pipe fixture
exited before contention. These are corrected proof defects, not native acceptance.
Pre-reconciliation detail is preserved in `/tmp/loo-672-before-realign-context.patch`;
the concept walkthrough remains `scratch/concept-review.html`.

## Concept review after the launch repair — November 4

Outcome: no further design change and no open product choice. The implemented
three-intent path now expresses Jack Heart's accepted interaction without asking
Desktop users to learn provider-thread or attachment machinery. Start and Take
back no longer collapse into the same launch authority. The earlier launch-race
counterexample above remains a regression requirement, not an outstanding redesign.

The reviewed interaction is: select a Task and reveal its existing conversation;
if none exists, offer explicit Start; if a competing client acquires input before
Start executes, preserve that holder and offer Take back on fresh observation.
When the holder leaves, passive Retry may reveal existing history. An absent
Session and a Session without native history share the Start experience while
Rust retains their different persistence facts. Failed reads remain unknown.
Close removes this host's warm intent and fences late preparation; a deliberate
return may reopen. Completed departure continues through LOO-717, with local
presence sent to the local owner and remote presence to its observed destination.

No additional Session lifecycle, stored readiness state, launch path or retirement
owner is needed. Fresh-load prefetch and saved restore use the same bounded reveal
queue. The usage text in `swift/README.md` already states the accepted behavior;
existing skill instructions need no change. Source and existing contention-test
logs support this review, not rendered native acceptance. No runtime tests were
rerun for this documentation-only step.

That review handed off to gate, matching app/CLI regeneration,
review publication and interactive re-demo. Native input/draft continuity,
Rex takeover, reboot recovery and the twenty-Task private footprint remain open;
eight warm conversations is still provisional. The current walkthrough is
`scratch/concept-review.html`. This review neither authorizes landing nor resolves
those empirical questions by assumption.

## Remaining verification and review

The screenshot correction and non-replacing Start are ready for review publication
to PR #1816. A matching disposable app/CLI candidate was regenerated with
`scratch/prepare-redemo.py`; use the current launcher in `loo-672-demo.md`.
Headless Desktop passed all 700 tests, including real-CLI Start and local
retain/departure/return, navigation, cached-to-fresh restart, DTOs, Close and warm
prefetch. The selected Rust run passed 225/226 tests; the stale-viewer fixture
was repaired to wait for a known viewer flag and expect Reveal, then passed alone.
Prepared-command contention, Start/Take back, lifecycle and transport checks passed
in the broad run. Clippy, formatting, architecture, Swift boundaries and 72 website
checks passed. Xcode's fallback `build-for-testing` also passed
(`/tmp/loo-672-publish-xcode.log`). Linux isolation remains with CI; native proof
remains with demo.

The initial Desktop run's stale expectations were corrected for saved selection,
completed departure and disconnected shells. Its sleep-based debounce test failed
in the broad run and passed unchanged alone; the flaky test was removed as AGENTS.md
requires. Remaining persistence tests pass. Logs:
`/tmp/loo-672-publish-rust.log` (initial failures preserved),
`/tmp/loo-672-publish-stale-viewer.log`,
`/tmp/loo-672-publish-swift-recheck.log`,
`/tmp/loo-672-publish-debounce.log` and `/tmp/loo-672-publish-clippy.log`.

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

Inspected compression logs: `/tmp/loo-672-compress-intent-{rust,swift,swift-recheck}.log`.
The Rust prepared-command run passed five tests. The Desktop run built the app
and passed 92 of 93 tests; its sole failure was the old positional Start mock,
which was corrected and passed in isolation. This is not a clean full-suite rerun.
Earlier launch checks remain in `/tmp/loo-672-start-*.log`; pre-compression notes
are preserved in `/tmp/loo-672-before-compress-context.patch`. Gate owns affected
suites, Clippy, formatting and review publication; native usability and memory
remain with demo. This reconciliation changes documentation only.

Checks: `git diff --check` and `lf context` — passed; existing `cargo test -p loopflow --test transport_tests prepared_ -- --nocapture` — 5 passed; `scripts/test_desktop.sh --jobs 4 --filter 'SessionsStoreTests|WorkspaceSessionLifecycleTests|WorkDestinationTests|WorkCLISubscriptionTests/missingTaskConversationStartsExplicitly'` — 92/93 passed, corrected Start mock passed with `--filter 'SessionsStoreTests/startsOnlyTheSelectedSession'`; no tests rerun during realign.


Prior review publication at `df07fd9b5f` passed 220 affected Rust tests, CLI build,
Clippy, architecture and formatting checks against parent `b81df60579` after a
conflict-free sync. Its broad Swift run was incomplete: the runner stopped for
syspolicyd host pressure, not a product assertion. Linux isolation remains with CI.
Full commands, logs and contrary evidence are preserved at
`df07fd9b5f:scratch/loo-672-ready-conversations.md`; they do not establish current
native usability or a measured memory budget.
