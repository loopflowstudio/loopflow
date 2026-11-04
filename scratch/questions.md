# LOO-498 — design simplification review

**Build authorized, November 4:** Jack Heart approved implementation after
clarifying: LF cleans up Desktop-owned shells only; it does not close Rex shells.
Retained Rex terminals for Task Sessions prevent Task cleanup regardless of focus.
Confirmed final removal releases the Rex hold subject to normal cleanup conditions;
unknown host state is not release. Remove Rex away detection and native settlement
from the implementation plan. Continue the existing feature Graph to demo, without
Auto-Ship or merge approval.

The smaller design is recorded in [rex-workspace.md](rex-workspace.md). Native
movement/template copies replacing named-copy management remains an unaccepted
optional proposal; preserve existing behavior during this build. Passive arrivals,
Wave defaults, Task view, agent host routing and native-split cwd remain required.

**November 4 accepted direction:** Jack Heart selected one-way template launching.
LF layout state belongs to Loopflow; Rex may consume a best-effort rendering but
cannot write its edits back. Rex owns native mutations, persistence and restore.
This supersedes the older full-restoration question below; see the current
ownership correction in `rex-workspace.md`. The durable host-ID/observation
records still need an implementation audit; no layout mirroring is intended.

**Demo correction, November 3:** [workspace-demo.md](workspace-demo.md) records
that the configured LF opening failed at database-schema compatibility. The
raw native-split API probe does not prove the app's Split button omits cwd;
that UI path remains unverified. Do not require a scope reduction on the raw
API evidence alone. A separate native split-demo session is available for review.

Jack Heart requested documentation first. The primary conversation drafted
[the user guide](../docs/workspaces.md) before further feature implementation or
publication. It describes the agreed experience and explicitly marks current
delivery gaps; use it to reconcile the product contract. Do not silently weaken
native split, restore, agent routing or lifecycle requirements to match the branch.

Jack Heart's November 3 build approval supersedes the mistaken-launch stop.
The current scope, implementation and remaining work are in `rex-workspace.md`;
no separate Task-view scope, Auto-Ship or merge approval was added.

The former question about Rex away detection is superseded: retention of a Task
Session terminal is sufficient. No foreground-use signal or Rex retirement policy
is required. The remaining engineering question is reliable terminal-to-Session
association through host disconnect/restart, without treating unknown as closed.

**Native split capability review, November 3.** Jack Heart's native-split
requirement is contradicted by the installed Rex revision-8 API: new native
shells anchored to two different Task directories in one session both start in
the isolated HOME. `scratch/rex-host-probe.py` reproduces this through
`session.new_split` and each new shell writing `pwd -P`. Public creator options offer
per-block cwd; the method inventory offers no per-Task/window default setter.
Account-wide Lua configuration is outside the accepted design. Dependent work
stops at this capability boundary pending a supported host change or Jack's
design judgment; no narrower acceptance is assumed. The independent passive
arrival fix now starts its Task viewer in the validated Task checkout.

The same isolated probe confirms native children on a custom server have
REX_SESSION/REX_BLOCK but no REX_SERVER. Current-viewer placement remains the
right path for recorded viewers; unknown custom endpoints cannot be guessed from
opaque block IDs. No running-app/socket scan or global host configuration was added.

Historical November 3 LOO-717 coordination required native settlement. Jack
Heart's November 4 decision above supersedes that requirement for Rex. Inspect
the current shared lifetime implementation and integrate retention only; do not
reintroduce native Rex closure from historical API notes.

**Implementation dependency checked November 4:** LOO-717 remains open in
PR #1811 at demo. Its current `rust/loopflow/src/transport/README.md` still
requires `--native-settled` on every presence write, and native resource closure
before checkout retirement. This checkout has no Task presence API. Integration
cannot satisfy the newly accepted no-Rex-settlement boundary through that
contract. The shared owner needs to provide retention/release without assigning
Rex closure responsibility to LF; no parallel retention store or cleanup loop
was added here. Independent locator/creation simplification proceeds.
