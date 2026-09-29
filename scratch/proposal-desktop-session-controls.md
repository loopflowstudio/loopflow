# Proposal: Desktop Session controls

2026-09-29 · LOO-298 · Prepared for Jack Heart. **Private patch only: unapplied,
uncompiled, no behavioral tests run.** Main owns integration, builds, Git and
all ongoing source work. This finite contribution writes only this artifact and
`.lf/tmp/desktop-session-controls-proposal/`. No provider, installed Home,
process control, PM/Git mutation or additional worker was used.

## System understanding

The accepted contract, remaining-work, Session-wire research and existing Desktop
ancestry handback were read before source inspection. This proposal extends the
current controls; it does not repeat the inventory audit or change pagination.

### Architecture and data flow

`RegistryQuery` reads a complete repository Session inventory; `PodiumModel` keeps
last-good readings and fences stale refreshes. `SessionsView` reconciles those IDs
into `SessionsStore`, releasing surfaces and panes for removed records. The
navigation projection is disposable; real Session IDs own terminals and drafts.
`session bind` resolves durable Task identity, then uses the existing transaction
for same-target retry, permanent assignment, ancestry and Flow compatibility.

### Evidence and counterexamples

- Filtering the inventory for display would make `SessionsView` release hidden
  Sessions and `refreshSessions` clear their selection. The proposed display
  filter does not enter either path.
- Task detail, its single-Session shortcut and breadcrumb sibling menus also
  reveal conversations. Filtering only the sidebar would leave headless Sessions
  discoverable by default; the patch covers these navigation consumers together.
- `SessionsView` previously chose its terminal layout from `taskPath`. Binding an
  unbound conversation to a Task with another checkout could therefore replace
  the visible layout. Selected conversations now use the existing retained
  `WorktreeNodeView`/focused workspace; Task overview and Task files keep their
  existing Task placement. No path supplies ancestry.
- `task_status` can reconcile PR state; it is unsuitable as an inert confirmation
  lookup. Session actions/DTO expose no resolved arbitrary historical Task label.
  The small preview below uses the existing bind resolver instead.

These are source observations, not reproduced runtime failures.

## Proposed behavior

**Headless discovery.** Add `--interactive all` alongside unchanged true/false
and the unchanged CLI default true. It maps to the existing `SessionFilter` None
mode, using one complete SQL selection. Desktop requests both modes with limit0,
then defaults its outline, Task conversation list, single-Session shortcut and
sibling menu to interactive. The existing outline menu gains **Show headless
Sessions**. The toggle changes navigation only: hidden selected conversations,
complete inventory, panes, draft and refresh membership remain intact. An already
selected hidden conversation stays named in its breadcrumb. `--all` still means
all repositories; no new ordering, page envelope, history filter or lifecycle.

**Permanent bind.** The Session breadcrumb gains **Bind to Task…** for a Session
without Task ancestry. Enter an issue identifier or stable Task ID; **Review
assignment** calls `session bind --dry-run --json`. Its four required fields are
session_id, task_id, identifier and title, mirrored in Swift and one shared fixture.
The preview is resolved identity only: no assignment, Started, permission claim,
provider launch, PR reconciliation or Task reopening. Ordinary command observation
may still record an Exec. Confirmation names the Session ID and resolved Task
ID/title, states permanence, then calls the existing bind owner with the stable
Task ID. A changed label cannot redirect that ID. No roadmap membership or launch
eligibility is needed, including done/off-roadmap Tasks.

The actual transaction still decides Wave/Flow compatibility and rejects another
Task; preview does not reserve eligibility or duplicate these checks in Swift.
A refusal retains the exact target/error for retry. A pending confirmation remains
reachable even if a poll observes the assignment after a lost response; same-target
retry remains the store's no-op. Already-bound Sessions have no new bind control.
There is no reassignment/unbind implementation. Successful readback updates typed
ancestry and invalidates older polls without replacing Session or terminal IDs.
Async work retains its original repository/draft owner, following rename's pattern.

## Tensions and review

One complete both-mode read costs more than the previous interactive-only read.
This is deliberate to preserve inventory semantics without two racing selections
or hidden-pane pruning. It is not a dense-data or paging solution; those remain
main's coordinated discovery work. The existing unlimited read and enrichment
cost remain visible. No new permission DTO or lifecycle action is invented.

Review corrected the Task-checkout layout dependency, hidden sibling/Task entry
points, strict built-in UI fixture arguments, an inaccessible Rust test import,
and inherited one-line text truncating confirmation. The patch keeps confirmation
text multiline. Surface focus after popover close and rendered behavior remain
unproved until the mounted check executes. No native retry or provider machinery
was changed.

## Patch, source identity and validation

Patch: `.lf/tmp/desktop-session-controls-proposal/desktop-session-controls.patch`
SHA-256: `1dddcf4634ee8f20aa22e4c105377433abe008f06b9fd0393afb7b85f537d246`.
**18 files / 40 hunks.** Apply hunks only; private full files are never
integration replacements. Opening/closing HEAD: `915aa82ffda9dd7a87408c087e38a2febe3d3cf1`.
Main's preexisting Exec work and later checkpoints were not modified. Closing
comparison found no drift among captured source/context files.

`initial.json` contains every inspected source/context SHA-256 and original bytes
live under `original/`. `validation.json` records each proposed-file SHA-256,
format result, closing comparison, hunk count and production measurement. No
source bytes changed during validation. All serialized hunk counts and old contexts
were checked independently; sequential replay equals proposed bytes; each existing
hunk context is unique. Read-only `git apply --check` passes against the current
checkout. Rust private files pass rustfmt check. Swift private files parse/format
successfully through Xcode's swift-format into separate private output; this is
not Swift type checking or a style-clean claim for the retained repository style.
JSON parses. **No cargo/swift/Xcode builds or product tests ran.**

Production delta: **+258 / −35 = net +223**.
Line SequenceMatcher, autojunk disabled, against captured originals; trailing Rust
test modules, test files, SessionFixture.swift, JSON and Markdown excluded. No
renames. This is proposal cost, not branch reduction or performance evidence.

## Behavioral proof for main

The private patch includes focused tests, all unexecuted:

- Extend existing public binding proof: preview leaves Work/Started/provider launch
  count unchanged, then bind by the returned stable ID. Preserve original usage,
  different-Task refusal and same-target retry. Extend the existing done-Task
  case with preview while retaining done status and its original assertions.
- Extend public inventory: true remains default; all returns both local modes;
  --all separately adds foreign repositories. The multi-origin import compares
  one mixed-mode inventory with the retained true/false inventories.
- New `SessionControlsTests`: shared required-field wire, hidden selection/rename
  draft, inert preview, exact off-roadmap ancestry, an older held poll, and rejected
  bind with retained-target retry. These simulate CLI responses, not provider or
  native terminal behavior.
- Extend `namedSessionDrillDownRetainsTerminal`: existing native surfaces/draft
  survive headless toggles and binding while Task planning names another checkout.
  The existing final PTY assertions remain. This drives the bind control/model;
  final preview/commit are model calls, not proof of every rendered popover button.
  A rendered review of target text, Cancel and confirmation remains needed.

Suggested focused commands in main's serialized isolated runner (not run here):

```sh
uv run python .lf/tmp/cut-i/run.py desktop-controls-rust cargo nextest run -p loopflow --lib --test session_cutover_tests --no-fail-fast -E 'test(binding_preview_requires_exact_identity_and_label) | test(binding_starts_the_task_once_without_reattributing_prior_work) | test(continuing_provider_children_inherit_the_bound_session_without_rewriting_history) | test(inventory_scopes_before_paging_and_keeps_worktree_repository_identity) | test(import_stores_each_old_session_once_with_its_name)'
uv run python .lf/tmp/cut-i/run.py desktop-controls-swift swift test --package-path swift --no-parallel -Xswiftc -gnone --filter 'SessionControlsTests|SessionRenameTests|PodiumModelTests|RegistryQueryTests|WorkspaceNavigationTests'
uv run python .lf/tmp/cut-i/run.py desktop-controls-mounted swift test --package-path swift --no-parallel -Xswiftc -gnone --filter 'WorkspaceNavigationProofTests/namedSessionDrillDownRetainsTerminal'
uv run python .lf/tmp/cut-i/run.py desktop-controls-clippy cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Regenerate/check docs through the existing website workflow after the included
CLI-doc hunk; the patch includes no generated HTML. Require actual executed Swift
Testing results, not an XCTest zero count. Keep same-target/store races and
Flow-incompatible binding proofs; run further affected checks only for a changed
boundary or failure. No configured Desktop/provider acceptance, completed cutover,
dense performance, installed conversion or Task/Flow disposition follows.

## Open questions and handback

No unresolved product choice blocks these two controls. Full completed-history UI,
Session/Exec wire retirement, provider continuation, bind-time usage allocation,
paging and dense performance remain outside this contribution. The prospective
usage assumption is unchanged. Main owns all integration and proof from this
handback; leave the artifact uncommitted for reconciliation.
