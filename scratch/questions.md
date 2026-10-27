# LOO-634 questions

- 2026-10-27, resolved from source: moving Graphs at a step boundary needs no new
  policy choice. LOO-502 (Jack Heart, 2026-10-17) removed ordinary pinning; the
  main-Home resolver returns the install entry gate per step, so a live Graph's
  next step already runs on the selected lf. The earlier "LOO-107 pinning vs
  moving Graphs" question rested on a stale premise. Remaining work is the
  disposition and the across-switch proof in the plan.
