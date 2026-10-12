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
The implementation is checkpointed through `10dce0b1cd`; the earlier uncommitted-work
description has been superseded by these checkpoints. All targets below are removed.

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

- Installed recovery of the two reported Sessions and real Codex terminal
  interaction remain unproved. Stand-in CLI tests establish source behavior.
- Installed acceptance retains both reported IDs: `e9fc442cdda14454aa65f5d16d98bd7f`
  and `2030b31b03704d0191762b53b32f22fe`. Retrying the retired first Session must
  retain completion and the successor's primary selection. A still-live provider
  must remain singular; missing identity remains unresolved, not dead.
- Publication and landing remain with the supplied Flow's later steps.

Publication review corrected the prior foreground-close test: live providers remain untouched; exact observed death is now accepted without mutating process or attachment records.

Gate review found no further production-code mismatch. The first broad run retained ambient account selection; its account failures passed with inherited `LF_*` cleared. A Machine version mismatch also passed on the clean rebuild. The release-repair rerun exposed a cached fixture helper referencing the removed materialized checkout; rebuilding that helper restored the test. TESTING.md records this recovery. The full Rust run exhausted its 1,200-second phase budget after 2,500 of 2,555 tests completed; its remaining coverage belongs to CI, not a claimed local pass.

Checks: `uv run python scripts/test.py --reuse-passing` passed architecture, rustfmt, all-target Clippy and 75 website tests, but Rust timed out; network-isolated `cargo nextest run -p loopflow` filtered to `provider_account::`, `add_alias_rename_connect_and_remove_preserve_identity`, `release_run_repairs_failed_checks_before_tagging`, `session_lifecycle_tests`, `session_record::runtime::tests`, `ops::human_session::`, and `store::sqlite::sessions::` with inherited `LF_*` cleared and `TMPDIR=/private/tmp` passed 110/111, with the remaining release test passing via `cargo test -p loopflow --test release_tests release_run_repairs_failed_checks_before_tagging -- --exact` after `cargo clean -p loopflow-test-support`; full Rust completion and the disposable-Linux-account test remain CI-owned, installed terminal acceptance remains with demo/review.
