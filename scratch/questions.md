# LOO-498 — remaining design judgments

**Reconciled November 4, 2026.** Jack Heart approved the revised build: LF templates
flow one way into Rex; Rex owns native edits, shells and restoration. A retained Rex
Task Session terminal prevents cleanup regardless of focus. LF cleans Desktop-owned
shells only. Rex away detection, native closure and settlement are superseded.
Build/review publication through the authored demo is authorized; Auto-Ship and
merge are not. [rex-workspace.md](rex-workspace.md) holds the plan and remaining work;
[docs/workspaces.md](../docs/workspaces.md) is the documentation-first user contract.

- **Native split defaults:** the installed revision-8 raw split API starts shells
  in HOME for two source Task directories when cwd is omitted; no scoped default
  setter was found. The app Split action remains unverified. Accepted scope still
  requires native shortcut/menu/API splits in the source Task worktree/Machine.
  Supported host capability is an engineering dependency; any narrower contract
  needs Jack's decision. Account-wide Lua and an LF-only split substitute are not
  accepted solutions.
- **Named copies and handoff:** dropping durable named-copy reuse and coordinated
  destination preparation remains an optional, unaccepted simplification. Existing
  behavior and the unfinished handoff requirement remain in scope.
- **Retention integration:** LOO-717's current `b81df60579` implementation still
  requires native settlement, contrary to the revised Rex contract. Its shared
  lifetime owner must support retention/release without LF closing Rex shells.
  Reliable terminal-to-Session association across disconnect/restart is unfinished;
  unavailable inventory cannot release a hold. No new product judgment is needed
  to preserve that accepted boundary.
- **Bare-open endpoint discovery:** native custom-server children supply session/
  block IDs but not REX_SERVER. Unqualified `rex whoami` can identify another server.
  The November 4 isolated probe confirmed that native shells descend from the serving
  Rex process. Implementation now reads that ancestor's explicit listener, then
  validates the caller block; no endpoint scan is introduced. Current-viewer
  placement remains authoritative for agent tools. Unreadable ancestry fails with
  `--server` guidance before any configured-default query. Configured agent
  resumption/move and discovery without readable server ancestry remain unproved.


Implementation choice, November 4: remote arrivals reconcile every fifteen seconds
through the existing Machine request path, in addition to local Work changes.
The destination alone decides whether a remotely owned Task started; an absent
local checkout is not evidence of remote backlog. This polling interval is a
reversible adapter choice, unrelated to the fifteen-minute lifetime policy.

[workspace-demo.md](workspace-demo.md) preserves the failed configured opening,
later disposable real-data demonstration and native Split review still owed.
Merged installer work in #1815 changes the available repair path, not those past
observations or proof of current configured behavior. Synthetic Tasks and manual
observer-socket wiring are supporting evidence; Jack requested real Work for review.
