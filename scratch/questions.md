# LOO-672 — Accepted decisions

Jack Heart selected:

- November 4 re-demo: “but the conversation should be auto stated and opened”
  and “thats teh whole pointof this task”. Working Tasks automatically establish
  and open their primary conversation on load/selection, including absent Session
  and missing native history. No Start/Open click. This supersedes the earlier
  explicit-Start rule; it was the wrong constraint for this experience.

- October 28, 2026: return at the bottom; exact scroll position need not survive release.
- November 2: land infrastructure first, then redesign on delivered behavior;
  save net machine resources while keeping active Task switching fast.
- November 4: keep recent unfinished Tasks warm, accepting memory cost for
  instant input; completed Tasks still release on departure through LOO-717.
- November 4: takeover without Fork, preserving last-screen + explicit Take back.
  Shared input was considered and not selected. Navigation never transfers input.
  CLI fork remains independently available on the same Task and checkout.
- November 4: “make sure to prefetch working tasks on load” extends discovery
  beyond saved visits to fresh idle/active/moving Tasks, using bounded preparation and warmth with non-replacing ownership.
- November 4: “ok, proceed” authorizes implementation and review publication
  through interactive demo, not shipping, merge or installation. The accepted
  stack parent is LOO-717/#1811. The stopped November 2 implementation was an
  operator error, not prior work to replay.

Jack Heart set sixteen warm unfinished conversations across repository windows
(November 5, “make it 16”). Engineering assumptions, not choices attributed to
Jack: visible-pane exceptions, selected-first restore and two concurrent preparations. Memory and usable-input measurements
remain outstanding. Native readiness uses the live relay event separately from
saved frames and program status; configured-provider readiness remains unproven.

Historical pre-infrastructure proposals (detached quota, old budget, Claude-only
Fork and snapshot construction) are superseded. Their full source is preserved at
`a32c385044:scratch/research/loo-672-pre-infrastructure-plan.md`.

Implementation assumption, November 4: reuse non-replacing Start for automatic
working Task openings; keep explicit history-only reveal for other Work. If an
input was already consumed but no native history survived, navigation does not
replay it; the pane's **Try again** or `lf resume` does. The current headless run verifies preparation and ownership; rendered native acceptance
remains with the Graph's demo step.

Implementation assumption, November 4, superseded November 5 (sixth concept review):
automatic startup was limited to owned Tasks by analogy to "nothing moves an
unassigned Task". Jack Heart's "auto open everything" and "once you've selected
something, you want to see its session" cover started unassigned Tasks, and the
reading pass claims nothing, so `conversationIntent` is now working → Start and the
Wave board's backlog rule is untouched. Unstarted Tasks still start explicitly:
placing their checkout starts the Task, which opening never authorizes, so their
Session slot offers Rust's Start. To exclude unassigned Tasks again, restore the
`!isUnassigned` term.

Concept-review assumption, November 4: a Task primary's first turn with no person's
request reads and reports, whether Desktop opened it or someone ran `lf resume --task`.
This drops operating without a greeting on that explicit path, because the launch cannot
tell the two apart without carrying a new bit through the transport. A person is present
there and one message starts operation. To keep it, pass automatic opening into launch.

Concept-review assumption, November 4: steers delivered with an automatic first turn
are reported, not acted on. Jack excluded replaying old Task input through navigation,
and the Session cannot tell a fresh comment from history at that boundary. A comment
written just before load therefore waits for the person's first message there; Graph
steps still receive it. To act on fresh steers, a delivery rule must separate them.

Implementation assumption, November 4: only new checkouts get the created marker. Task
checkouts made before this fix have none, and a Task can adopt a hand-made checkout, so
marking them all would trust folders lf never created. Each such checkout without a trust
entry shows Claude's dialog once, reported as Available. This checkout is one of them. To
remove that one-time dialog, record placement strategy on the Task and backfill from it.

Concept-review assumption, November 4: a consumed first launch with no history recovers with
one click, not automatically. Its capture may hold only the generated reading pass, so another
attempt would replay nothing Jack wrote, but retrying on load relaunches a dead provider on every
refresh, across up to eight Tasks. So a reboot or quit before the provider writes history costs
one **Try again** for that Task. To make it clickless, allow one automatic retry per Desktop run
for a Task primary with no stored request.

Concept-review assumption, November 4: a conversation that finished its automatic reading pass
counts as Available like any other idle conversation. So after a load, up to sixteen Tasks appear in
the sidebar's Available filter and as available on the Wave board, though nobody asked them anything.
Availability is judged in Rust from provider evidence, and a report is waiting there. To keep the
filter for conversations a person has engaged, exclude a Session whose only turn is the automatic one.

Historical Start-click correction evidence:
`3ef57e857394d5a97271ea00b6048ad60c22bf9a:scratch/questions.md`.

November 5 re-demo clarification from Jack Heart: started working Tasks have no
normal “unopened” conversation state; background prefetch initializes and opens
the native conversation before selection. Preparing Session metadata alone does
not meet this requirement. See `loo-672-load-prefetch.md`.

Implementation assumption, November 5: a prepared Session's terminal starts from the
store, sized to the last mounted pane or 1100×700 before any pane has mounted, so the
provider's first frames use a plausible grid; the pane that shows it resizes it.
The launcher is an explicit `GhosttyManager?` dependency (the app passes `.shared`;
tests and headless registries pass none), so a prepared fixture never spawns a real
`lf` in a test. `.hidden()` never attaches NSViewRepresentable views to a window, so
no hidden host can launch; launching from the model replaces it.

November 5 review expansion from Jack Heart: the repaired Task behavior “otherwise
looks good”; repository and Wave conversations must also open on selection and
prefetch. “auto open everything *AND* prefetch everything”. No normal selected
No session/Open session gate. Supersedes the Task-only and started-only discovery
restrictions. Accepted scope and constraints: `loo-672-load-prefetch.md`.

Implementation assumptions, November 5 (primaries):

- Repository and Wave conversations are warm outside the sixteen-Task bound, not
  inside it: a repository has one plus a few Waves, and counting them against the
  Task bound would evict working Tasks to keep Waves warm. Close still releases
  them; a deliberate visit readmits. To bound them too, move them into `recentTasks`.
- Load order is selected Work, visible tiles, repository and Waves, then recent
  working Tasks. Primaries are a handful of fast prepare calls, so they delay the
  first Task by well under a second; put them after `recentTasks` in `warmWork` to
  let Tasks lead instead.
- `lf resume` and `lf resume --wave` with no message now open with the same
  reading pass as Task primaries, so the operator waits for the person's first
  message there too. The same trade-off recorded for Tasks on November 4 applies: the
  launch cannot tell Desktop's automatic opening from an explicit CLI opening.
- Repository and Wave conversations are prefetched only in the loaded repository's
  window; Desktop never loads other repositories on the Machine.
- The queue verifies a primary's Machine but not its checkout, because Rust places
  primaries in the scope's persistent agent worktree, which Desktop's navigation
  location (the repository root) never names.

Implementation assumptions, November 5 (eighth-review tile direction):

- A tile opens in place while a view plan is presented and the tile is visible,
  not "when it is not the selected Work": on a Dashboard the focused tile is the
  selected Work, so that reading would have left the Dashboard. An entered tile
  (`.entered`) keeps the pane's navigating Retry.
- The primary tile's "No primary Session" text, which offered nothing, also became
  the shared placeholder; its button prepares the primary in place through the
  registry's `openConversation(in:store:)`, the one opener behind automatic
  opening, the tile and the primary's selected pane (compress removed the
  workspace's own `openPrimaryConversation`, which skipped the Machine check).

Concept-review assumption, November 5 (ninth pass, landed): "a tile opens in place while
a view plan is presented" means any presented board, including the repository page and a
Wave view, whose Task tiles `presentWaveTiles` opens like a Dashboard's. `inTile` keys on
`selectedViewPlan` or `navigation.waveView` plus visibility, so those tiles' Retry attaches
in place and the board stays. Wave-view tiles navigating instead was the rejected reading.

## Sync onto LOO-717 viewer retention — November 5

LOO-717 replaced host occurrence/presence reports with Task Session viewer
holds (its `loo-717-shell-lifetimes.md` is authoritative). Resolution: warm
conversations keep their viewers attached (`SessionsWorkspace.presented`), so
visible and bounded recent unfinished Tasks hold their checkouts; leaving the
warm set, Close and window close detach with Ctrl+]. Presence reporting, quit
release and the retention tests built on them were deleted. Assumption: a
minimal `DesktopAppDelegate` stays only to fence window teardown on quit, so
quitting does not erase saved warm intent; it reports nothing.
