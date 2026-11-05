# LOO-498 — current design and remaining judgments

**November 5 interactive concept review supersedes the older account below.**
Jack Heart selected config-owned Wave templates, existing DB-owned sticky Desktop
Task layouts, no LF persistence of hosts/workspaces/occurrences anywhere, and a
fresh native workspace on every `lf open`. Returning and restoration belong to Rex.
Repeated-open and named-copy reuse are no longer requirements. No replacement
sidecar registry or durable creation receipt is allowed. See the current opening
section of [rex-workspace.md](rex-workspace.md).

Jack also clarified native hierarchy: repository **or** Wave → Rex session;
Task → tab; workspace → that tab's splits. Fresh opening does not require another
native session when the current/explicit target is available. Jack clarified that “loop” was voice-to-text noise; there is no extra concept.
Jack also requested known-host detection and checking the same model in Herdr and
cmux. Installed APIs support the mapping documented in rex-workspace.md; full LF
rendering and configured proof remain. The stated working interpretation is that automatic arrivals use normal configured
Task splits instead of special previews; retain this as an interpretation until
reviewed, not as a verbatim option selection. No further choice about repeated
opening is pending. Configuration uses existing
shared/private scopes; exact layout syntax is an implementation proposal. The
outstanding engineering question is how retained native Task terminals and live
arrivals meet their accepted contracts through host/Session evidence without LF
host records. Unavailable inventory cannot authorize cleanup. Other execution,
input, Machine and directory guarantees remain. The concept review itself changed no implementation or execution. Jack Heart then
requested “ok, pursue”; implementation and review publication now continue through
the pursue skill, without Auto-Ship or merge. Full configured layouts on arrival
are the stated reversible implementation choice for that pass.

---

## Earlier judgments and implementation choices

The prior persistence and reuse assumptions below are historical and superseded;
keep their independent evidence and accepted protections.


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
  app window, and Jack was using the Machine (again November 5: this run sat inside
  his live Rex with active input). Both LF entry paths now have native-Rex cwd
  evidence on an isolated server; the app's Split itself stays with the demo. Open: which source the app prefers
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
- **Leaving reads `ended`:** Ctrl+], a finished conversation and a host restart all
  read `ended`, because only the lock's absence is known; `failed` is reserved for a
  reported attachment error.

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

**Concept review second pass, November 5:** `lf open` text output stops claiming
"a retained view keeps this Task" while no cleanup consumes the hold; it reports
the inventory (`present · confirmed closed · unreadable`). JSON `rex_hold` stays.
Exposing the hold in `task status` is left to LOO-717's API rather than chosen here.
Reversible wording.
