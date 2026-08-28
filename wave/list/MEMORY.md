# Open questions

- Does exact OS-verifiable `ProcessOwner` identity hold sole authority for Task
  liveness and signaling, or can provider Invocation state authorize or veto
  control actions?
- Does release promotion stop and restart the current Task process, or allow
  adjacent releases to coexist under an explicit ownership boundary?
