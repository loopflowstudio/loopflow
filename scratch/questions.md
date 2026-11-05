# LOO-498 — remaining design judgments

**Reconciled November 5, 2026.** Jack Heart approved the revised build: LF templates
flow one way into Rex; Rex owns native edits, shells and restoration. A retained Rex
Task Session terminal prevents cleanup regardless of focus. LF cleans Desktop-owned
shells only. Rex away detection, native closure and settlement are superseded.
Build/review publication through the authored demo is authorized; Auto-Ship and
merge are not. [rex-workspace.md](rex-workspace.md) holds the plan and remaining work;
[docs/workspaces.md](../docs/workspaces.md) is the documentation-first user contract.

**November 5:** Jack Heart requested fixing directory tracking and native splits
for automatic and manual entry. LF's side is implemented; see Native split defaults
in [rex-workspace.md](rex-workspace.md). No scope reduction, global configuration
change, Auto-Ship or merge is authorized.

- **Native split verification:** Rex's app Split was not driven. It needs a focused
  app window, and Jack was using the Machine. Open: which source the app prefers
  when both exist (LF makes them agree), and whether it accepts LF's OSC 7 host.
- **Assumption, reversible:** a conversation bound after launch reports its selected
  directory on later attachments, though its running provider keeps the original
  cwd. Jack's requirement is that splits follow the Task; `lf session bind` already
  documents that later connections use the selected directory. Reporting the
  provider's own cwd instead is a one-line change.
- **Assumption, reversible:** a viewer already attached when a bind happens does
  not move until it reattaches.
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

Implementation choice, November 4: a live Conversation observer offers Ctrl+R to
take input only after the existing execution owner acknowledges its attachment.
This keeps preparation and explicit transfer on one destination connection; no
resume command runs during native creation. Ctrl+] continues to leave the viewer.
The shortcut is a reversible interaction choice, not a change to named copies,
native ownership, or the required configured handoff demonstration.

Implementation choices, November 5, all reversible:

- **Fourth readiness state:** the plan named connected, failed and uncertain.
  `history` was added because a Session with no live owner has nothing to connect
  to; calling that failed would offer a pointless retry on most idle Tasks. It gets
  no retry, and `lf resume` continues it. A pane showing history does not upgrade if
  the Session goes live later.
- **Retry shape:** `--retry-observers` adds a new pane rather than restarting the
  retained one, so its error stays visible and nothing replays. Repeated retries
  after panes fail accumulate panes; Rex owns closing them.
- **Exit status:** `lf open` exits by native results only. A failed observer is
  reported in the receipt without failing the command. Readiness-gating scripts
  read `conversations`.
- **Leaving looks like failure:** Ctrl+], a finished conversation and a host restart
  all read `failed` ("no longer running"), because only the lock's absence is known.

[workspace-demo.md](workspace-demo.md) preserves the failed configured opening,
later disposable real-data demonstration and native Split review still owed.
Merged installer work in #1815 changes the available repair path, not those past
observations or proof of current configured behavior. Synthetic Tasks and manual
observer-socket wiring are supporting evidence; Jack requested real Work for review.

**Concept review, November 5:** retention hold evidence is a Task occurrence's
native destination present in a successful Rex inventory; observer liveness stays
readiness-only. Settled from Jack Heart's confirmed-removal/unknown-is-not-release
contract, not a new decision. Reversible if the demo shows restored, forgotten tabs
holding completed Tasks is unwelcome. Readiness `failed` for a stopped observer
becomes `ended` (naming only).
