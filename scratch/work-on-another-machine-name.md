# Share Task planning across machines — LOO-412

## Accepted direction — Jack Heart, 2026-10-09

Jack Heart selected one planning transport per repository: Linear, shared Git,
or a personal user-keyed Git ref (the default). Personal configuration supplies
defaults; checked-in repository configuration applies automatically to everyone.
Every process reads and writes its own local planning. Each connected machine
reads/writes Linear independently. A laptop going offline cannot make its worker
depend on a designated publisher. Linear outages leave changes buffered locally,
with no Git fallback. Git still carries code in all modes.

Jack requires one typed transactional planning writer for local and imported edits.
Planning state travels; Workflow positions, Sessions, Processes, checkouts, control
and local delivery attempts do not. Machine assignments and exclusive first-start
scheduling are outside this Task. `--machine` dispatches the selected command.

This supersedes callback routing, a designated Linear publisher, mixed Git/Linear
receipt exchange and manual per-Wave enrollment. No master election, resident,
cross-version compatibility, automatic turn/Flow retry, hidden argument or imported
herdr/cmux implementation. Publication is for Jack's review only; no landing,
installation or real planning export to the public repository.

## Configuration and experience

```yaml
# .lf/config.yaml, or ~/.lf/config.yaml for a personal default
planning:
  provider: git
  remote: origin
  shared: team
```

Omitting `shared` selects `refs/loopflow/planning/users/<key>`; the first connection
creates the key. Recover the same key on a fresh second Machine with
`lf planning key --recover <uuid>`. Shared refs need no personal key. Repository
`planning` replaces the personal value as a whole. `planning: {provider: linear}`
selects Linear; `pm.linear_team` supplies its Team. An existing Team binding selects
Linear when explicit `planning` is absent. Configuration selects the whole repository
plan and its retained history; ref separation is not access control.

```sh
lf --machine mini task create --title "Fix the parser"
lf --machine mini --task <selector> skill implement
lf planning status --json
```

Cold lookup acquires from the configured transport. Repetition reuses saved Task
and checkout identity. Linear issues select existing provider mappings, preserving
independent legacy IDs. New Linear Task IDs derive from the provider UUID; locally
created Task UUIDs are reused for provider creation. Git preserves authored IDs.

Source code requirements travel separately: branch, pushed commit and selector.
Fetch precedes decisions. Missing/unpushed source work names the branch/commit;
a retained target checkout missing that commit asks for `lf sync` and keeps its
bytes. The origin's Work declaration is not forwarded. An unpublished Linear Task
stays local until provider creation is observed; Git is not a back channel.

Short commands save first, then attempt selected delivery. Linear attempts have a
five-second bound; Git subprocesses retain their own deadlines and effect locks.
Foreground CLI/Desktop/Session lifetimes keep exchange active. Disconnection
retains pending work; reconnect acquires before publishing. No agent turn is replayed.

## Implementation

`store/sqlite/planning_write.rs` now defines typed planning edits and owns writes,
creation and causal capture in the caller's transaction. Local creation/edit,
comment, completion, ordering, selection/rotation and provider acquisition use it.
Git import uses the same writer without minting echo mutations. Delivery receipts
remain with their existing local owners. Execution columns are absent from the
portable schema; imported planning creates no placement or delivery attempts.

Git retains immutable field mutations, causal parents and losing values. Accepted
heads are distinct from retained mutations, so failed projection cannot become a
local save's parent. Independent records project under savepoints, followed by Wave
selection. Reused mutation identities and foreign repository ownership reject the
transaction. Import and publication receipts remain distinct; uncertain pushes need
readback. Configuration activates only its selected destination.

Removed: mixed-provider merge precedence, cross-machine Linear observations,
creation/deletion/order receipt transport and settlement, `planning_creations`,
identity-association recovery, SQL JSON capture triggers and their field mirrors,
manual connect/use/select/associate commands, peer-only record construction and
their exclusive tests. Common LOO-406 Linear receipts and recovery remain.

## Remaining proof and review

- Gate owns the full affected suites, release-frontier materialization and headless
  Desktop checks. Existing Linux TLS fixtures own connected public Linear acceptance;
  real two-machine SSH and installation remain unobserved. Source checks use the CLI
  built in this worktree with disposable stores and simulated providers/transport.
- Configuration switches retain prior history and uncertain effects. They do not
  transfer another Machine's delivery attempts. Automatic migration of previously
  Git-imported, unmapped work into Linear remains an open design question; no
  exactly-once or seamless switching acceptance is claimed.
- [One-page +/- review](pr-review.html) and its source manifest now describe this
  reduction. The installed CLI has no `lf screenshot` command, so visual capture
  is unavailable. Publication is review-only; do not land.

Review findings repaired: repository selection replaces the personal provider
configuration; Linear rejects Git-only fields; automatic Session placement needs
pushed repository configuration in the fixture. Follow-up source lookup must retain
both local and provider IDs after export. Rejected selection history survives without
becoming the causal parent of a later local save. All imports exclude execution.

Checks: network-isolated built test binaries pass 33 focused tests (Git exchange/foreground, remote dispatch, config/setup, typed writer, Linear fields/export and follow-through); `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` and `git diff --check` pass. Full affected suites, headless Desktop and connected TLS acceptance remain with gate/CI.

## Earlier evidence

The complete mixed design and prior acceptance are retained at
`ae96102580e442fe582dd17ce5e6fef9b8cee69d:scratch/work-on-another-machine-name.md`;
that model is superseded, not proof of this cut. Original adoption demo: `0cd8e7f14`.
Mixed receipt recovery: `e37099e50`/`231e75898`. Native Codex exchange used synthetic
Responses and retained a live Session plus a subsequent turn; its Workflow was
seeded, not a running edge. It proves neither configured accounts nor the new route.
Release's operation-entry lesson still applies: a helper-only pass is not dispatch.
