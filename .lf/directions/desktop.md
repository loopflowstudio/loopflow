# Native workspace boundaries

- Current Wave navigation consumes `lf ls --all --current --json`; the shared
  CLI owns lifecycle filtering. `lf ls` without `--current` retains registry
  history. `lf roadmap --all` must not inherit a launching process's Wave.
- A worktree owns its terminal layout. The window owns native terminal
  surfaces; never mount one surface into two outer worktree slots.
- Conversation subject and terminal location are independent: use typed Work
  binding for context and actual provider-client terminal identity for local
  attachment. A matching cwd does not establish either relationship.
- Session actions must remain reachable for both direct Session panes and
  shell-attached clients. Complete the shared Session without closing its shell.
- A Run resumed interactively is a Session even when its original launch was
  headless. Keep launch provenance; the retained provider-client namespace records
  interactive history and its client receipts establish liveness. Resolve declared
  issue/slug subjects through shared Work binding before projecting Session links.
- SwiftPM tests exercise Ghostty. Also compile the Xcode app/test targets,
  whose terminal fallback does not import Ghostty-only types.
