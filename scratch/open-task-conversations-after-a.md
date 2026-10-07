# Open Task conversations after a failed provider launch (LOO-409)

Jack Heart authorized diagnosis through landing without a person-dependent review.

## Diagnosis and repair

The first retained input is ca3b3cf9be254e118f314fef934cd2e9, before the two failed
retries in the brief. Exec 390f3905-4dff-4c82-889c-73a9298e8bc0 ran task/session,
recorded generation-1 reserved and spawn_requested receipts, and exited successfully.
No native thread or durable provider process survived. Current terminal launch writes
only a temporary client receipt, removes it after wait, and never records the provider
process. The two later failures correctly refuse that unknown state. An initial
reservation/generation mismatch is not supported by the observed history.

Repair terminal launch evidence at its actual spawn/wait boundaries, reuse the same
admission checks for native resume, and retain uncertainty across interrupted startup.
Preserve identity, native history, active ownership and old unknown records. No schema
change or live SQLite editing. LOO-408 owns completion; LOO-324 owns account/history
lookup; LOO-400 owns terminology. Their responsibilities remain unchanged.

## Remaining proof and delivery

Reproduce with a disposable Home and finite fake provider through public CLI/PTY;
cover first open, pre-spawn failure, retry, existing history and active/unknown owners.
Run focused tests and affected gate/static checks, reconcile memory, land with completion.
Installed recovery requires published repaired code and adequate retained authority;
source merge alone cannot recover the affected legacy Session.

## Review findings

Atomic reservation closes the interruption gap between driver claim and launch evidence.
Native fallback previously bypassed the ownership checks; it now shares admission.
A second observed race lost successful native-history publication between opener polls;
readback now checks that history after a successful child exit.
A destructive fake-executable test initially fell through ambient PATH to installed
OpenCode, which returned Session not found and exited. The fixture now isolates PATH
and HOME; no successful provider conversation was observed in that failed run.

Checks: focused Session recorder tests 50 passed/1 installation-only ignored; public CLI
suite 12 passed; Task regression and affected gate remain pending.
