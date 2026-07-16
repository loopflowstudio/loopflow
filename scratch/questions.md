# Open questions / blockers — W2-176

## Blocker: `lf task acknowledge` cannot reach the registry (environment, not work)

The installed `lf 0.11.1` refuses the shared store:

```
database migration 0.11.009_profiles is unknown to lf 0.11.1
(latest known 0.11.013_task_review_state)
```

Root cause is the known cross-branch migration-number collision (wave memory):
this branch's tree numbers `profiles` as `0.11.011_profiles`, but the shared
`~/.lf/loopflow.db` was written by the divergent auth branch (#943) which minted
`0.11.009_profiles`. No 0.11.1-line binary — installed or built from this branch
— matches that ordinal, so every registry read/write (`lf task acknowledge`,
`lf roadmap`, `lf runs`) errors out on this machine.

**Impact:** the directive-1 acknowledgment could not be recorded via
`lf task acknowledge W2-176 --directive 1`. The acknowledgment *content* (how the
plan changed) is captured in the design note and in the commit message. Proceeding
per headless doctrine: report the blocker once, continue on the computable seed.

**Assumption:** the orchestration server that launched this Task Session runs a
binary matching the DB and can record acknowledgment/advancement; the local shell
`lf` cannot. This does not block the design or the registry-independent slices
(PR 1 `lf handoff list` is pure `cargo`-testable code).

## Verification gap: live Product Wave census not run against the real registry

The contract's proof includes "the real Product Wave census agrees with CLI
identity and counts." `ActiveSessionsCensus` is verified exhaustively against the
mixed fixture (`ActiveSessionsCensusTests`, 8 tests), and the view compiles and
loads from `lf roadmap` + `lf runs` + `lf handoff list`. But the live comparison
could not run: the same migration collision makes `lf roadmap`/`lf runs`/`lf
handoff list` error on this machine's shell. When the registry is reachable
(matching binary), the check is: open Control from a Wave header and confirm the
Active Sessions groups/rows match `lf roadmap` + `lf runs` + `lf handoff list
--active` output by identity and count. The projection is pure, so a fixture
match is strong evidence; the live run is the remaining confirmation.

## Decision taken (reversible): compose in Swift, complete one contract in Rust

Considered a single unified `lf sessions --json` census projection in Rust vs.
composing existing `lf roadmap` + `lf runs` + a new `lf handoff list` in Swift.
Chose composition: the directive frames this as a Mac presentation Task consuming
*already-merged* contracts, and the proof ("real Product Wave census agrees with
CLI identity and counts") reads as the Mac assembling the same reads the CLI
prints. The only Rust addition is exposing the already-shipped
`Store::list_interactive_handoffs` through the CLI — a contract completion, not a
new projection. If review prefers a unified Rust census, PR 2's
`ActiveSessionsCensus` logic ports there with the Swift side thinned to a decode.
