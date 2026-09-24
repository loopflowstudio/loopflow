# Native workspace boundaries

- Public planning navigation targets repo → Wave → Task → Session. Each Wave's
  current chapter Project is internal; do not add a Project or Chapter tier.
  Read the shared chapter summary, direct Tasks and unavailable Task evidence.
  Historical Project-bound Sessions use shared Wave ancestry. Retained Task
  evidence during chapter transfer belongs to repository/window navigation.
  Refresh that retained evidence when planning changes; selection-time data can
  be stale by the next transfer. Cancel historical-reference lookups when the
  human navigates elsewhere so late results cannot replace the chosen workspace.
  Task runtime exposes its current parent once through `project_id`; chapter
  transfer changes that parent, not a separate successor-routing identity.

- Current Wave navigation consumes `lf ls --all --current --json`; the shared
  CLI owns lifecycle filtering. `lf ls` without `--current` retains registry
  history. `lf roadmap --all` must not inherit a launching process's Wave.
- A worktree owns its terminal layout. The window owns native terminal
  surfaces; never mount one surface into two outer worktree slots.
- Retained Task pane choices include content identity: a reused pane ID can
  contain another Task's Session. Monitor selects its own AppKit responder;
  setting first responder to nil can return input to a visible terminal.
  Focusing a direct Session belonging to another Task must not overwrite the
  selected Task's saved choice; compare its shared subject before saving.
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
- A Run resumed interactively is a Session even when its original launch was
  headless. Keep launch provenance; the retained provider-client namespace records
  interactive history and its client receipts establish liveness. Resolve declared
  issue/slug subjects through shared Work binding before projecting Session links.
- SwiftPM tests exercise Ghostty. Also compile the Xcode app/test targets,
  whose terminal fallback does not import Ghostty-only types.
