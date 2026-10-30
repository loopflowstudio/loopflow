# LOO-694 questions

- Assumed: confirming viewers over the owner socket only on the one-Session
  path (prepare/resume, `lf open`), not on passive pages. Probing every live
  owner per Desktop refresh costs up to 1 s per hung owner. Rows may read
  `active` in the sidebar until the pane is shown, which re-prepares and
  reattaches. Revisit if the sidebar label matters.
- Assumed: "viewed" means a controlling viewer is attached, the notion `lf open`
  already used. Observer-only attachments (no current product path creates one)
  read as detached.
- Could not read Desktop's own log for the failed open: no `ui.log` since July and
  no `studio.loopflow` unified-log entries. Diagnosis rests on the store row,
  process table and the prepare reply.
