# Native workspace boundaries

- Public planning navigation targets repo → Wave → Task → Session. Each Wave's
  current chapter Project is internal; do not add a Project or Chapter tier.
  Read the shared chapter summary, direct Tasks and unavailable Task evidence.
  Historical Project-bound Sessions use shared Wave ancestry. Retained Task
  evidence during chapter transfer belongs to repository/window navigation.
  Refresh that retained evidence when planning changes; selection-time data can
  be stale by the next transfer. Cancel historical-reference lookups when the
  human navigates elsewhere so late results cannot replace the chosen workspace.
  Chapter membership comes from the shared Wave plan; Task runtime does not
  carry a second parent or successor-routing identity.

- Keep the compact outline in Loopflow's existing visual language: burgundy,
  cream, shared serif/sans typography and the operational Wave lens. Simplifying
  navigation does not require replacing those choices with generic styling.
  Design toward “A calmer desktop workspace”: a readable Work overview by
  default, then a Session view. For now, collapse Session with context and
  Session alone into “Session”; omit Description from the Session surface.
  Sessions have names and belong to the same repo → Wave → Task → Session
  hierarchy. Selecting a Session drills down from its Task; the breadcrumb
  provides navigation back to the Task and Wave. The selected Session name
  continues that breadcrumb, rather than adding a parallel view-mode hierarchy.
  Distinguish Sessions belonging to an active Flow (name and current step) from
  independent Task conversations; provider liveness alone cannot imply membership.
  Session names start with the invoked skill, or the historical magical musical
  animal generator for raw Sessions, and are editable in place. Basic operating
  guidance should replace generated names when a clearer topic emerges; preserve
  human-assigned names through refresh, later suggestions and concurrent writes.
  Keep exact Session identity, draft and retained panes through every depth.
  Conversation location badges are not part of the accepted list presentation;
  disclose any necessary opening/transfer choice in the selected conversation.
  Optimize the default for one repository: repo scope in the header, its data
  immediately below in the connected sidebar, and search anchored at the bottom.
  Omit a redundant repo root while scoped; retain explicit repository switching.
  Task rows show inline open-Session counts and open the single exact Session
  directly; several Sessions lead to the Task overview for a named choice.
  The accepted prototype uses the same ancestry in its breadcrumb, not a second
  sidebar Session tree.
  The sidebar working set contains started Tasks, independently of open Sessions.
  Clicking a Wave presents its complete plan, including unstarted Tasks, in the
  middle pane. Inspection alone does not start work or add a Task to the sidebar;
  do not add a separate All tasks sidebar shortcut.
  Canonicalize every repository source before forming outline roots.
  Task content starts with a compact title and linked issue identifier. Avoid a
  generic Open header, duplicate oversized title or internal preview diagnostics
  in the reading flow; keep missing evidence truthful without inventing counts.
  Task titles use the shared sans-serif face; retain the separate Wave typography.
  Label the Task's Linear description “Description” in the UI.
  Keep Task comments collapsed by default. Descriptions explain the current work;
  comments carry dated planning and execution updates. Current blockers and
  binding scope remain visible in the description. Render the stored fields
  honestly; do not disguise description text as comments in presentation.
  Accepted Task header treatment A: breadcrumb contains the clickable parent
  Wave followed by the issue ID, which links to Linear. Show the full Task title
  only in the page heading; omit the issue ID beside that title. Do not repeat the Task title in the breadcrumb or show the preview
  label “Linear snapshot · read only”.
  Task-page studies distinguish three situations: no Runs started; execution
  underway, paused or blocked without an open Session; and one or more open
  Sessions. The first needs a Start action, a preview of the selected Flow,
  progressively disclosed Flow changes, and collapsed comments. Remove “Local
  work evidence” from the Task page. The human prefers C's connected Flow map
  for the unstarted Task. Place collapsed Comments below Description and show
  their count. Clicking the selected Flow name turns it into a searchable
  typeahead; selecting a result updates the diagram and restores the name.
  Omit a separate Change flow button. A preview must not infer an unstarted
  Task from missing evidence. Between the Flow diagram and Description, show
  the unstarted state and absence of runs when established (study wording:
  “Not started · No runs yet”).
  Flow previews must support authored backward edges. The first/loop/finally
  arrangement becomes one ordinary Flow definition, not three UI configuration
  slots. Show explicit return targets and distinguish human revision from agent
  iteration. Blocked requests help; it is not another navigation edge. Counts
  describe observed passes without implying a pass limit. Keep a chosen Flow
  and its pinned execution definition distinct once work has started.
  Present Feature as one continuous connected path. Splitting Task design and
  Pursue into labeled sections made it read as multiple Flows; inline their
  steps and attach return arrows directly to the revisited steps.
  Give the repeated span a soft rounded background with “Loop” centered above
  its steps; omit an Iterate label on the return arrow. The region represents
  the authored loop, not another Flow. Nodes show literal lowercase skill names
  in monospace, with explanations revealed on selection. Human steps have a
  pale yellow fill and a matching border for pending human steps, without a
  person/You marker. Completed steps turn pale green, including human steps.
  Reserve the marker above a node for its execution status.
  Execution colors: Running and the Loop region/return path are blue;
  completed steps are green, pending human steps yellow, and Blocked red.
  Paused stays neutral. Use the same colors in the execution summary/history.
  Put the current iteration in the loop header (“Loop · Iteration 3”),
  distinguishing it from completed traversals. Use “iteration” consistently
  in the execution summary/history. Do not show a number before execution
  starts or imply a fixed maximum.
  On started work, hover or keyboard focus at the Flow name reveals Stop &
  restart. Search for the replacement, then confirm; choosing a result alone
  must not silently replace the active Flow. Preserve existing work/history.
  Put New session beside the Task title for unstarted, executing and
  Session-bearing Tasks: a plain conversation
  in the Task's worktree with Task context, independent of Flow execution.
  Latest human correction accepts Feature's second decision loop after demo.
  Render the real pinned definition: both decide and decide_delivery return to
  implement, with demo between those distinct loop-decide occurrences. Design
  stays outside the loops. Preserve per-edge traversal evidence; repeated skill
  names do not identify an occurrence. The earlier single-loop prototype is
  visual inspiration, not the current topology contract.
  Build the native Flow diagram from that topology using the accepted visual
  language; discuss the final two-loop composition once the working build is
  ready, as the human requested. Do not introduce a new approval gate.
  Final Advance leads to queue → land; final Iterate returns to implement.
  Distinguish queue admission, waiting/check failures and authoritative merge.
- Before presenting a configured demo, align its CLI, app and selected Home
  through supported development promotion and verify real chapter reads.
  An incompatible development Home and a missing current chapter are distinct
  failures. Preview chapter dispositions before applying planning changes;
  never conceal missing plans with fixture rows or direct database edits.

- Current Wave navigation consumes `lf ls --all --current --json`; the shared
  CLI owns lifecycle filtering. `lf ls` without `--current` retains registry
  history. `lf roadmap --all` must not inherit a launching process's Wave.
- A worktree owns its terminal layout. The window owns native terminal
  surfaces; never mount one surface into two outer worktree slots.
- Task selection derives from shared Session membership (one open Session
  enters it; otherwise the overview), never from a remembered pane ID: a reused
  pane can contain another Task's Session. Monitor selects its own AppKit
  responder; setting first responder to nil can return input to a visible terminal.
- Conversation subject and terminal location are independent: use typed Work
  binding for context and actual provider-client terminal identity for local
  attachment. A matching cwd does not establish either relationship.
- Session actions must remain reachable for both direct Session panes and
  shell-attached clients. Complete the shared Session without closing its shell.
- Join Sessions to Runs through required `run_id`, never Session-ID parsing.
  Ask/Flow publication prepares the Run before launching its client; preparing
  identity does not prove liveness. Session listing remains read-only.
  Release preparation locks before waiting on a resumed provider conversation;
  metadata/open requests must remain usable while its client waits for input.
- Native active-Run discovery uses existing per-Run client receipts, including
  clients launched before a new CLI build. Capture interval bindings establish
  generic Exec-to-Run identity; they are not a prerequisite for native discovery.
  An OS environment locator is optional evidence: macOS omitted LF_RUN_DIR for
  an owned live client in the discovery proof. A transient discovery cache needs
  a reader that survives observations; caching inside each one-shot CLI request
  still repeats cold discovery. Prove notification-loss recovery before polling.
  An ownerless capture can outlive its native client: retained native receipts
  resolve that ownership even when the client is dead. Dropping dead candidates
  must not turn that capture into a new missing-ownership warning. Keep the
  ownerless capture's namespace dependency in the bounded unresolved set: loss
  of its last native receipt must restore the warning. Cache that presence between
  ownership events instead of rereading dead native history on each tick.
  Manifest failures belong to the current live-client observation; do not retain
  them in the receipt-discovery cache after the client disappears.
- A Run resumed interactively is a Session even when its original launch was
  headless. Keep launch provenance; the retained provider-client namespace records
  interactive history and its client receipts establish liveness. Resolve declared
  issue/slug subjects through shared Work binding before projecting Session links.
- SwiftPM tests exercise Ghostty. Also compile the Xcode app/test targets,
  whose terminal fallback does not import Ghostty-only types.

- RegistryQuery's active observation is window-owned after first Monitor demand.
  Navigation never restarts it; configuration replacement drains the old reader
  and clears its evidence. Keep pipe decoding off the main actor and bound both
  frames and pending delivery. Recovery retains last-good rows; a fatal discovery
  frame ends the subscription with its original reason, rather than waiting for
  a transport timeout to overwrite that reason.
  On reader exit or stdout closure, drain pending stderr before reporting the
  failure: pipe callbacks can arrive out of order and hide the CLI's diagnosis.
- `scripts/test.py --loopflow` forces the fallback build in addition to affected
  suites. For an implementation-only compile, the existing runner can execute
  only `build_plan([], False, {"loopflow"})`'s enabled plans through `run_plans`;
  this keeps resource preflight and command supervision without broad tests.
