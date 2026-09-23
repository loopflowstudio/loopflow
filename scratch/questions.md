# Main workspace design — open decisions

- Conversation model: optional, bounded conversations with at most one current per subject; encourage fresh starts. Proposed controls are Start conversation, Continue, Finish conversation, and Start fresh. Shared lifecycle/replacement recovery and coexistence with mandatory Ask/FlowStep decisions need design; finishing a conversation must not imply completing its Task.
- New work: `loopflow.lf-new` proposes Task-first creation/preparation before bound design. Use that as the proposed ordinary implementation entry; Project selection at repo scope remains open. Existing unbound checkouts/Sessions still need access and a non-destructive association path. Keep Sessions-only window removal conditional on equivalent reachability.
- Workspace integration: consume `lf-new`'s outer worktree slots and inner terminal layouts. One current conversation per subject must coexist with free shells/manual agent launches and distinct repo/Wave/Project conversations sharing main; checkout identity is not Session ownership.
- Layout: the human favors making both A and D available. `main-view-round-two.html` demonstrates an explicit show/hide work list control. Full-width focus is only the mockup's initial default; determine which presentation opens first and whether that choice survives app relaunch. First-round studies remain available.
- Navigator resolved: one annotated Task list with minimal collapsible Wave/Project headings. Hierarchy organizes the list without dominating it. The draft proposes expanded groups initially, compact collapsed counts, and expansion-preserving search.
- Breadth: the user explicitly wants all Waves/Projects; whether the default must span multiple repositories is not yet specified. Draft starts with the selected repository and preserves repository switching.
- LOO-291's existing proof requires a human-selected external workflow. None has been named; do not infer one or treat Loopflow self-hosting as that proof.
- Remaining scope: directive editing, shared Session actions/display path, bounded conversations, required workspace integration, external trials, and measured budgets remain required before claiming LOO-291 complete. The unified-navigation first slice is implemented locally; configured native proof remains open. No PM scope change or publication occurred.

## First-slice implementation decisions — 2026-09-23

- Use full-width on first open; retain the show-list preference, search, expansion,
  and selection per repository/window for the visit. No app-relaunch persistence.
- Compared the canonical main checkout's SessionRecord and SessionsView with this
  prepared checkout: identical. Rust and Swift expose no shared Session actions
  or display path yet. Reuse the existing native pane/actions without adding a
  new legality matrix or changing the wire contract. LOO-284 integration remains.
- The lf-new checkout's SessionsView is also identical. Nested worktree layout
  and live shell attachment are still designs, not consumable implementations.
  Retain the existing workspace owner in this cut; do not create another registry.
- Preserve every existing Session. Unmatched Sessions remain an explicit recovery
  group in the unified list; this is not an invented repo association or Unfiled
  planning kind. No current-conversation cardinality enforcement in this slice.
