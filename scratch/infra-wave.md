# Recover retained landing obligations safely

Execution decision, October 3, 2026. Jack Heart authorized autonomous incident
repairs and delivery in [the original report](launch-release-incident-report.md).
[The causal analysis](launch-release-5whys.md) supplies evidence and its limits.
This is the working design for the next implement step in this checkout.

## Selected core

Maintainers must be able to reconcile retained deliveries after upgrading,
without one formerly valid `home` supervisor preventing every pending landing
from loading. Repair the released-data transition end to end: migration,
domain reads, supervisor fencing, and supported reconciliation of retained
delivery intent. Keep this core here; no replacement worker or checkout.

`lf task status --json` reports `this checkout has no Task`. This is a taskless
incident contribution, not a new Task assignment. No Wave was supplied for new
allocation; do not infer ownership from the branch or the affected product Tasks.
The incident Flow already continues through implement and compress without a
human step. Do not start pursue or feature, which would introduce review gates.
Publication and landing are authorized, but are subsequent delivery operations;
this planning step neither performs nor claims them.

The reader in `rust/loopflow/src/store/sqlite/pr_landings.rs` rejects `home` even
though released migration `0.12.14.001_release.sql` admitted it. The current
release operation therefore cannot reliably enumerate retained obligations.
Implement a single migration draft using `uv run python scripts/new_migration.py`
and the repository's released-frontier convention. First inspect the current
schema and all supervisor writes to settle retirement semantics.

Preferred approach, subject to that inspection: retire obsolete supervisor
authority transactionally, advance its fencing generation, retain the landing
and delivery intent, and preserve historical placement/identity evidence in its
existing durable owner. A historical Home PID must never become a local PID.
Do not blindly relabel `home`, delete rows, overwrite failures, or operate on the
live database with ad hoc SQL. If old executable writes are not fenced by the
existing generation contract, resolve that before enabling new claims. Avoid
adding a parallel recovery registry or a generic retry object.

Acceptance:

- A released-frontier fixture containing local and Home-supervised landings
  migrates and loads through the real domain reader. Include pending obligations
  and terminal history, not only SQL-column comparisons.
- Preserve landing IDs, Task/PR references, requested and observed heads,
  after-merge/next-PR intent, states, failure evidence and operation references.
  Preserve local supervision; obsolete authority cannot heartbeat or settle
  using its old generation. Historical Home PIDs are never signaled locally.
- Repeat migration/open/reconciliation safely. Unrelated pending deliveries
  remain readable. Use controlled provider facts to verify reconciliation
  without making GitHub writes from a fixture.
- Focused migration/store/reconciler tests establish the contract. Run build
  sanity in implement; affected suites and required lint belong to gate. Record
  one-line check results, not a duplicated ledger.

Installed release recovery remains a separate acceptance claim. Before any
v0.13.0 re-entry, inspect exact authority and terminal outcome of retry
`ff94aac9-b59d-409b-9bce-45dc921f1f1a`, then refresh GitHub publication facts.
No process-list absence establishes permission to overlap a publisher. Recover
through supported lf operations only after authority is settled; retain the
same release identity and prove a terminal result without duplicate publication.
If authority remains unknown, report that blocker while shipping the proven
source repair through the authorized delivery workflow.

## Independent follow-up: reliable detached worker startup

Maintainers need a Task worker to start in its requested checkout even when
tmux's server cwd has been deleted, and a failed start must expose a useful,
secret-safe cause instead of only a ten-second timeout.

This outcome can ship independently of landing persistence. Allocation and
launch are deferred because no owning Wave/Task was supplied and this checkout
has no Task. Reuse matching existing work once ownership is established; do not
attach this infrastructure repair to LOO-371 or LOO-372 merely because they
exposed it. File a short brief with these acceptance requirements and stage this
design plus both evidence documents before launching an autonomous Flow.

Tentative implementation: establish shell-quoted cwd inside the detached child
before exec, retaining the existing authority/environment scrubbing. Keep
bounded startup failure evidence under the existing launch identity, without
dumping environment values, secret-bearing argv, or unrestricted child stderr.
Prove an isolated tmux server with deleted original cwd can launch into an
existing checkout, including paths with spaces/quotes, and admit the worker.
Prove missing cwd produces an actionable failure. Do not alter global tmux
settings, restart unrelated sessions, or lengthen the timeout as the repair.

The historical LOO-326 A/B probe was read at
`/Users/jack/src/loopflow.make-the-release-and-ci/scratch/loo-326-worker-startup.md`.
It observed tmux 3.7c launching children with unresolvable cwd despite valid `-c`;
explicit child `cd` restored pwd and an isolated server passed Task status with
the same installed binary/Home. Original timed-out child stderr was unavailable.
This supports prevention, not attribution of every LOO-371/372 timeout.

## Preserve current work

Fresh status inspection supersedes the report's LOO-371 continuation suggestion.
Its managed Flow `a3ff7e90-0ffb-4b05-9089-5945439cfc1e` reports idle, but its work
also contains current Flow `adf0d547-aba5-4c58-90af-bf491ab78d29`, unfinished
implementation Session `session_863145da4d694adb95de85e489448fa4`, and an
unfinished implement Exec. Do not resume the managed Flow into competing work.
LOO-372 still reports running implement in
`82c166ae-3233-4005-8437-e6926649be1b`; preserve it. LOO-366 reached its existing
review-design boundary; leave that unrelated review contract intact.

Review finding: a managed Flow's idle label is insufficient evidence of an empty
Task checkout. Use the full work association before future continuations. No
Task, Flow, PR, release, runtime data, or active worker was changed in planning.

Check: source/Flow inspection and fresh Task status completed; prose-only design,
so no build or test rerun. No Tasks launched; startup allocation and exact release
authority remain explicit follow-ups.
