# LOO-355 assumptions and remaining boundaries

2026-09-30 — Jack Heart requested composable lifecycle operations and action
coverage. This implementation uses `task pr ISSUE ACTION` to address the existing
PR parser, rather than duplicating all PR flags as new Task verbs. Start/resume,
interruption, Flow replacement, checkout recovery and dependent splitting retain
`run`, `interrupt`, `restart`, `checkout` and `create --stack-on` respectively.

Cancellation may retire an unclaimed review cursor. A live or unknown worker is
excluded. The existing abandonment-intent columns fence new worker claims before
Linear is contacted; a failed provider request retains that intent for retry.
No new lease table, migration or cancellation receipt format is introduced.

## Remaining implementation/design boundary

The requested completion/landing symmetry is not fully implemented by the new
cancellation path. Existing `task pr ISSUE land -c` composes authoritative landing
with Task/Linear completion, but landing still retains the Task checkout. Calling
`wt delete` unconditionally from that path would delete the checkout beneath the
Task worker executing its own delivery Flow. Resolve cleanup at worker settlement
before changing that path; do not add a self-wait or bypass ownership.

`task complete` still records success only after delivery settles. `task delete`
still trashes the issue while retaining authored work. The requested inventory
makes those distinctions explicit; it does not establish the stronger downward
composition for completion and deletion. Recovery currently means checkout or
saved-execution recovery, not reopening an abandoned outcome. These remain part
of LOO-355, not silently removed acceptance criteria.

## Live acceptance blocker

The selected `lf 0.12.26` rejects both `lf task abandon --help` and
`lf task sweep --help` with `unknown command`. Jack's retained infrastructure
constraint forbids running a source binary against the installed store or
promoting it from a Task. New Task operations follow the selected installation,
so private copied data cannot serve as installed acceptance.

Do not patch the installed database or cancel issues through a separate Linear
writer as a substitute. Ordinary delivery must make the new operation available
before its configured disposable-Task proof, live sweep preview/apply, and
LOO-309/LOO-329 closure. No installed Task or Linear outcome was changed here.
