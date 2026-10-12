# Open a Task's conversation

Jack Heart requested autonomous repair of lf-8fed729 on October 12, 2026.
The accepted outcome requires ensure → connect, replace → connect, recovery
of stranded interactive providers, and no duplicate live provider. No prior
design was supplied; this is the implementation design, reconciled October 12.
Launch timing remains an implementation assumption, not a separate product approval.

Prepare the primary Session and input in `ensure`/`replace`; `connect` owns
the foreground launch. Keep scope/launch locks, Task selection, native history,
workspace placement, and replacement transaction. No schema change.

October 12 read-only inspection confirms both reported Sessions lack native
history and `replace` retired the first. Explicit opening must retry a retired
Session too, preserving its completion marker and the successor's primary
selection. Replace input against the observed completion marker (reject races),
and permit publication of that exact capture. This matches existing explicit
opening of retired Sessions with native history; no reopening lifecycle is added.

An interactive provider with an exact recorded PID/birth observed dead may
settle under the existing attachment fence. A live or unknown interactive
provider cannot authorize resume, signaling, or another launch. Headless
process-group death rules remain unchanged. Death does not invent success.

## Implemented

Commits `d3a00bf0ca` and `fac6e65141` implement preparation and recovery.
The checkout is clean at `9ccaac2e48`; the earlier uncommitted-work description
has been superseded by these checkpoints. All targets below are removed.

- Primary `start` background launch: replace with input preparation.
- `serve-conversation` hidden command and its sole serving function; the unused
  `start_home_session` wrapper.
- tmux conversation liveness/start helpers, launcher name, mock launcher sets,
  and exclusive launcher/retry tests. Process evidence owns liveness; preserve
  selection, workspace, replacement, input and native-history coverage.
- Completed-Session refusal in explicit opening/explanation and input replacement;
  retain completion evidence and reject changes since the input snapshot.

## Review findings

Preparation now re-reads the Session after acquiring its launch lock so an
opener that won the lock cannot leave `ensure` publishing a stale input.
The retired/no-history case found in the actual reported records is covered
without clearing completion or stealing primary selection. Unknown native
identity stays unknown; no generic process reaper or alternate launch path was added.

Preparation accepts only an ID, keeping the locked read as its sole Session
snapshot. Input replacement validates completion once at admission; the SQL
writer runs in that same transaction and needs no second completion predicate.
The CI-repair caller also loads its Session inside the writer's transaction.

Reconciliation found no further bounded code mismatch. Main through `2f6945ed04`
is integrated, including on-demand planning acquisition and scoped agent guidance;
neither changes foreground launch ownership. Release's child memory reinforces
that operation-entry proofs and installed acceptance are separate: installation
of 0.13.13 does not establish recovery by this branch's later repair.

## Remaining

- Gate: affected Session/connection/store suites and automated acceptance;
  installed recovery of the two reported Sessions and real Codex terminal
  interaction remain unproved. Stand-in CLI tests establish source behavior.
- Installed acceptance retains both reported IDs: `e9fc442cdda14454aa65f5d16d98bd7f`
  and `2030b31b03704d0191762b53b32f22fe`. Retrying the retired first Session must
  retain completion and the successor's primary selection. A still-live provider
  must remain singular; missing identity remains unresolved, not dead.
- Publication and landing remain with the supplied Flow's later steps.

Checks: prior focused Cargo tests (9 passed), native-resume proof, fmt and Clippy reused for unchanged Rust; `git diff --check` and `lf context` pass after reconciliation; broader suites remain with gate/CI and installed terminal acceptance with demo/review.
