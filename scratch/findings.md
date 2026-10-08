# Findings supporting LOO-427

Dated source evidence, not additional requirements or a second plan. The
[implementation plan](compare-cmux-s-command-line.md) owns scope, sequence and
acceptance. [Open questions](questions.md) own unresolved product judgments.
The earlier implementation attempts below found the storage coupling; the October 8 cut removes it locally.

## Identity finding and existing owners

Source inspection at `7e852defe`, not a live-store experiment:

| Owner | Existing fact and missing behavior |
|---|---|
| [`CanonicalRepo` / `RepoId`](../rust/loopflow/src/repository.rs) | The first collapses local worktrees/symlinks to a Machine-local path; the second derives `owner/repo` from origin without its host. Neither establishes cross-machine correspondence. |
| [`Machine`](../rust/loopflow/src/durable.rs), [`add_machine`](../rust/loopflow/src/store/sqlite/durable.rs) | One optional remote directory, upserted by Machine ID. No local repository or pairing; adding another repository on that Machine replaces the directory. The shared repository root must also support repositories with no Tasks. |
| [`Machine dispatch`](../rust/loopflow/src/lf/commands/ssh.rs) | Uses the registered `Machine.repo` as remote cwd. Work-directed routing must resolve its delegated execution location. |
| [`PortfolioRepo`](../swift/LoopflowMac/PortfolioRepo.swift) | Local path and last-opened date only. |
| [`RegistryQueryLocal`](../swift/LoopflowMac/Services/RegistryQueryLocal.swift), [`WorkModel`](../swift/LoopflowMac/WorkModel.swift), [`SessionsView`](../swift/LoopflowMac/Views/SessionsView.swift) | Local subprocess reads and work stream, path-keyed Session readings, and a conversation-launch rejection for another Machine. Remote reading/opening still needs implementation. |
| `WorkspaceIdentity`, [`TerminalIdentity`](../swift/LoopflowMac/Services/Ghostty/GhosttyManager.swift) | Workspace identity includes Machine/worktree; terminal identity is only a Session or shell string. Carry Machine through surface lookup, callbacks and input. |

Retain `SessionsWorkspaceRegistry` for workspaces, `MultiplexerStore` for layout
and focus, `GhosttySurfacePool` for terminal views, and `RegistryQuery` for Rust
state/legal actions. AppleScript currently captures only the key window; there
is no general Desktop layout or terminal API. The comparison records the
installed lf 0.13.9 baseline separately from newer source vocabulary.

Rechecked at `7ee1247e8` on October 7: root `lf open` already rejects non-macOS
before invoking the app launcher; Work targets and terminal guidance remain
missing. `WorkLinkRouter` has one pending URL, overwritten by a second cold-start
request and consumed by the first window to register without checking repository
membership. The cutover must preserve requests for different repositories as well
as converge repeated opens of one repository. This is source evidence, not a
native concurrency trial.


#### Storage findings — October 7, 2026

`inherit_placement` copies the parent Machine at creation; `placement_in` reads
that row directly. Replacing this with nearest-ancestor lookup is unsafe:
`store/sqlite/children.rs::task_checkouts` joins the same placement row to the
Task's existing worktree, and `ops/task.rs::task_snapshot` uses its Machine to
decide whether to inspect that path locally. Changing delegation would relabel
an existing checkout as remote without moving it. Checkout-based Session
association would then use the wrong location. Authored
delegation and observed checkout location must be separated in the existing
storage owner before changing inheritance. Historical rows record no explicit
versus inherited provenance; matching a parent is not proof of inheritance.

There is also no repository Work root to use yet: `durable.rs::WorkRef` contains
only Wave, Project and Task. The current local import mints Project IDs in
`store/sqlite/planning.rs` and Task IDs in `ops/task.rs`. Shared Linear records
alone do not establish shared Loopflow identities.

The related Tasks do not close this gap:

- LOO-412 at `0cd8e7f14a6bffecd9323541e7689570316b892a` carries issue/branch/
  commit/planning input through SSH, never source Task IDs or authority. It
  preserves existing target IDs, Workflows and Project selection. Two existing
  Machines can therefore retain different Task IDs for one issue. Its checkout
  transport is reusable; its independent planning adoption cannot implement
  LOO-427's shared-tree contract unchanged. PR #1491 was published for review,
  not evidence of an integrated shared source.
- LOO-406's current brief selects a complete single-machine local lifecycle and
  explicitly excludes host callbacks, Git synchronization and multi-person
  authority changes. Its October 7 design, inspected beside source
  `510977d11a38b818e1d89da1be09e79d4215ce3c`, preserves established identity and
  leaves independent-machine import convergence to synchronization. Its local
  storage work must not be duplicated here.
- Jack Heart's LOO-406 comments `fc82810b-d506-4d02-a6da-c46ea79ea924`,
  `f929c7f5-c063-454e-93a8-7a6d7374402d` and
  `25b3eede-7d52-4c48-9fb1-8e21e0050590` distinguish disposable execution
  machines, a leaning toward Git as the official copy, and Linear-owned shared
  plans. They do not select an always-on SQLite master or a publication protocol.


## Decisions these findings do not establish

- Missing shared-source implementation does not reopen Jack Heart's accepted
  one-tree/delegated-subtree direction. It prevents slice 2 from choosing its
  physical authority contract, not slice 1 from separating recorded location.
- Removing the former SSH `--repo` option did not ban Work-directed routing.
- Matching a parent's Machine is not evidence that a historical assignment was
  inherited. Migration must preserve the observed meaning rather than invent it.
- Exact pane/content identity prevents redirection, not destructive changes to
  an existing unsent draft. Session input needs the LOO-387 interaction contract.
- A successful command receipt does not prove usable rendering. The cmux
  disposable-window probe is evidence only for that host, not Desktop parity.
- `documented_commands` tests ambiguity; `dto_fixtures` tests wire shape. Neither
  replaces behavioral storage/routing/native-model tests.

Current alternatives rejected by the accepted direction: repository-pairing
registry; independent per-machine planning trees merged by Desktop; clone-name
or remote-URL matching as Work identity; rewriting Machine defaults per request.

## October 8 implementation review

Checkout location now lives on `tasks`, using the existing `TaskCheckout` reader.
Review found two paths outside that reader: SQL membership matched only paths,
and comparisons bypassed file-location validation. Both now use recorded Machine
evidence. Unknown Machine evidence previously allowed local file reads and
missing-path Session association; it now stays unavailable, preserving explicit
bindings. No new DTO or parallel placement store was needed.

The first focused run failed on fixture setup (missing required `input_published`
and a nonexistent PR branch); both fixtures were corrected. Gate still owns
materialized/installation migration and complete Session lifecycle verification.
This is slice-1 evidence, not shared-source or native Desktop proof.
