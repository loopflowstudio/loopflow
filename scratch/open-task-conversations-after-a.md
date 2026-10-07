# Open Task conversations after a failed provider launch (LOO-409)

Jack Heart authorized diagnosis, verification and landing without a person-dependent review.

## Established cause and repair

Installed v0.13.9 is the exact base revision 6448e3c9e7. LOO-386 first captured
ca3b3cf9be254e118f314fef934cd2e9 under Exec 390f3905-4dff-4c82-889c-73a9298e8bc0.
It recorded generation-1 reservation, spawn-request and interactive-open receipts,
then successful driver exit, without native history or durable provider process
identity. The two inputs in the brief are retries. Missing initial reservation or
generation mismatch is unsupported; why Codex published no native thread is unknown.

Native terminal spawn now records exact process identity and waited exit; actual
spawn failure is positive non-start evidence. Reservation commits with driver claim.
Native resume shares initial admission checks; live/unknown providers remain protected.
A finite successful terminal publishing history between opener probes remains resumable.
No schema change. LOO-408 completion, LOO-324 account/native discovery and LOO-400
terminology stay separate; read-only inspection found no launcher overlap with LOO-408.

## Evidence and remaining delivery

Disposable-Home PTY proof with a fake Codex: original code reproduces the exact
no-connection/no-confirmed-exit error after first opening without native history;
repaired code reaches PROVIDER_READY and exits 0 through both session resume and
explicit connect --replace, retaining one Session and its native hook identity.
The first deliberately history-free opening remains an honest error.

Supported installed reconnect of session_7faf8ac130cb4cefb8f69d501808596f failed
again before provider admission, retaining input 3c603f15bbab4b68b3df141d2deac83e.
The Session remains unknown with the same Task and checkout. No live SQL edits,
installation, unrelated interruption or successful installed recovery is claimed.
Its old missing process evidence cannot be recovered by this source change.

Gate found two native-history tests that assumed admission recorded no observations;
assertions now select provider turns while retaining exact output/usage ownership.
A fake-executable test initially fell through ambient PATH to installed OpenCode,
which returned Session not found and exited. The fixture now isolates PATH and HOME;
no successful provider conversation was observed in that failed test.

Remaining: finish affected gate, publish and request merge with completion through lf land -c.
Checks: affected gate ran 2,280 Rust tests (2,278 passed, two history expectations corrected), architecture and website (78 passed/3 skipped); CLI/Task and PTY proofs pass; final history correction checks 8 passed and formatting/Clippy passed; CLI guide 4 passed.
