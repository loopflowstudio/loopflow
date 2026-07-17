# Open questions and blockers — W2-294

## BLOCKER: no local verification exists for this branch

**Host `syspolicyd` blocked the focused Cargo verifier before it reached `main`.**
`cargo test -p loopflow --lib task::runner::ci_fix_lifecycle_tests` exited 1 with
no test output — the binary never ran, so this is *not* a test failure and not
evidence about the code either way.

**Nothing in this branch has been observed green locally. No local pass is
claimed.** Per instruction, no fresh local target was rerun. GitHub CI is the
verifier of record for this PR: read `rust-test` and `rust-lint` at the
published head, not any local signal.

What that leaves unproven at publish time, for a reviewer to weigh:

| Claim | Status |
|---|---|
| The four fixtures compile | **Unproven locally.** CI `rust-test` decides. |
| Kickoff/Iterate/Gate/handoff fixtures pass with the fix | **Unproven locally.** CI `rust-test` decides. |
| Sabotage 1 (delete the `ci_fix_wake.is_none()` guard) reddens Iterate + Gate | **Unrun.** Requires a working local verifier; it is a deliberate break of main, so it cannot be delegated to CI on this branch. |
| Sabotage 2 (move the exit back below the `loop`) reddens Kickoff | **Unrun.** Same reason. |
| `cargo fmt` / `cargo clippy --all-targets` | **Unrun locally.** CI `rust-lint` decides. |

The sabotage proofs are the ones that establish the guards are load-bearing
rather than fixture-pinning, and they are the part CI structurally cannot
supply. They remain owed. If the host verifier is unavailable when this is
reviewed, the honest reading is: the fixtures assert the right facts *if they
run*, and only CI green establishes that they run at all.

## Assumption taken while blocked

`exit_bounded_ci_fix_turn` takes exactly 7 parameters, and clippy's
`too_many_arguments` fires at 8+, so the `#[allow(clippy::too_many_arguments)]`
I initially wrote was dead and is deleted. **Unverified** — if CI `rust-lint`
disagrees, restore the allow rather than reshaping the signature; the parameter
list is the runner's turn state, not a knob set.

## Not a question, a standing hazard for whoever rebases W2-308 onto this

`scratch/` does not survive `lf pr land`. The durable statement of this
boundary's contract is the rustdoc on `exit_bounded_ci_fix_turn` in
`task/runner.rs` — that is what a later rebase reads, and it is written to be
read that way. This file is scaffolding; that doc comment is the interface.
