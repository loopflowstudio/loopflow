# Open choices (923677)

Made during kickoff with nobody present; each is reversible and marked in
`plan.md`.

1. The brief says of the shared components "Use them; add none", and also
   "becomes the shared state component". No state component exists on main.
   Taken: `PaneEmptyState` is rewritten and renamed `StateView` in `Design/`,
   so the count of components does not grow.
2. The sheet's workflow popover (search field, a summary line per workflow)
   becomes an ordinary `Menu` of `StartFlowItems`. Search and summaries go.
3. File stays in the sheet though the prototype draws only Cancel and Start.
4. The catalog gains a required `workflow` field (steps and the "You take
   part at …" sentence, written in Rust). An app from this branch needs this
   branch's `lf`.
5. Compact dashboard slots lose their icon-only fallback; the action alone
   stays, with the title as its help.
6. The prototype's "A Task that has not started" state is placed in the
   unstarted Task's conversation pane ("Conversation not open" today); the
   workflow bar keeps its own Start.
7. The two prototype sections are not deleted in this change's first pass;
   they go once the Dev app check has compared the screens against them.
8. "Open its conversations" and "Open another repository…" are the
   prototype's proposals. Each ships only if the view can already reach that
   action; otherwise the state has one button.
