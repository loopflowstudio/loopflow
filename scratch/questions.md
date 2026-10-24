# LOO-570 assumptions (kickoff, 2026-10-24)

Jack Heart delegated these decisions (October 23). Each one is reversible; the
plan proceeds on the simpler choice.

- **Retention follows event type, not Session closing.** Jack proposed keeping
  stream lines only while a Session is open. On Jack's store 5,384 of 5,669
  Sessions never get `completed_at`, and no reader needs bodies for open
  Sessions (Available reads `session_activity`). So the store keeps fact
  envelopes for every Session and never keeps bodies.
- **Debug dumps degrade with Home rotation.** `lf history show --events` reads
  `runs/<capture>/events.jsonl`. Once `disk.rs` rotates that file (24 h grace
  when Home is over budget, as Jack's is), it prints retained facts only. No
  second copy is kept for the dump.
- **Inspection Processes expire after 7 days.** The rule is structural: a
  finished `lf` Process that nothing references (no child, attachment, Session
  event, Graph or step, Workflow or move, Wave activation, CI repair, Session
  transport or agent, relocation, steer). It is not a command allowlist. Task
  work listings and `lf ps` history lose such rows after 7 days. The 7 days is a
  judgment call: Home's 24 h grace is too short for "what ran yesterday".
- **Migration backups are removed when their install settles.** That covers
  every fingerprinted `loopflow.db.backup-*` generation. Hand-named backups stay
  under `disk.rs`'s newest-per-database rule.
- **Usage envelopes stay** (140k rows, 81 MB). The usage reducer takes the peak
  over every observation, so dropping intermediate observations changes results.
  Compacting them is a separate change.
- **Prune walks captures over 24 h old, from a cursor file.** An earlier lf
  still recording a capture past 24 h leaves its later lines until the
  follow-up removes the capture pass. Malformed legacy payloads are deleted
  with the bodies, so `evidence_gaps` falls for those inputs.
