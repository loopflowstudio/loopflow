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
| `cargo clippy --all-targets` | **Unrun locally.** CI `rust-lint` decides. |
| The inlined reduction even *compiles* | **Unproven locally.** One `cargo check` was attempted on the theory that it emits metadata rather than linking, and so might clear the block; it was killed with no output. Not evidence either way. CI `rust-test` decides. |

`cargo fmt --check` is the one local verifier that does run: it exits 0 with
zero output at this head. That is a formatting fact and nothing more — it says
nothing about whether the crate compiles or the fixtures pass.

The sabotage proofs are the ones that establish the guards are load-bearing
rather than fixture-pinning, and they are the part CI structurally cannot
supply. They remain owed. If the host verifier is unavailable when this is
reviewed, the honest reading is: the fixtures assert the right facts *if they
run*, and only CI green establishes that they run at all.

## Not a question, a standing hazard for whoever rebases W2-308 onto this

`scratch/` does not survive `lf pr land`. The durable statement of this
boundary's contract is the rustdoc on `settle_ci_fix_turn` in `task/runner.rs`
— that is what a later rebase reads, and it is written to be read that way.
This file is scaffolding; that doc comment is the interface.

The ordering is not defended by a name, so it is defended by a test: move the
`if let Some(wake)` branch below the lifecycle loop and the Kickoff case goes
red on a second provider turn. That is the only thing standing between this fix
and its silent return.
