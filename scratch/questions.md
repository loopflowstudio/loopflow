# Assumptions

- PR #934 remains the first serial slice. Typed references and send-state proof
  begin only after it lands and Loopflow rotates the Task branch.
- Evidence references mean explicit commit hashes and repo-relative file/line
  paths. They resolve against the Wave origin repository without adding a trace
  or transcript store.
- The existing durable POST response is the acceptance boundary: the server
  returns the journaled user turn independently of agent startup and trace
  capture. Offline queuing remains excluded.
