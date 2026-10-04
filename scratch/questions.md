# Open design questions

October 4, 2026 — Task conversation and background Flow simplification:

- At design/review, does a background segment exit and return its next-step context
  to the Task conversation, or remain suspended until that conversation responds?
- Resolved by Jack Heart: recovery after process exit or crash belongs to the caller,
  normally the Task Session. Automatic checkpoint resumption is not required.
- What observation retention is required after a Flow ends? Preserve comparable
  graph/output history; choose its storage owner without recreating a controller.
