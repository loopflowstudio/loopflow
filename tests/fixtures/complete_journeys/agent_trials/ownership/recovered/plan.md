# Definitions ownership

Draft implementation plan, 2026-11-15. Selected journey: CLI skill lookup.

## Outcome and architecture

The CLI prints `Finish the selected journey.` for `implement`; unknown names
exit 2 without stdout. Library lookup retains `None` for an unknown name.
`src/runtime.rs::skill` currently owns the sole built-in body. The CLI and
`tests/cli.rs` import it directly. Move that unchanged implementation to
`src/definitions.rs::skill` and cut both consumers over together. No stored
state or data migration exists in this fixture.

Direct ownership is simpler than a re-export or adapter, which would leave
runtime as a competing lookup authority. Success means identical CLI behavior
with one definition owner. The principal failure risk is leaving a test or
production import behind; compile all targets immediately after the cut.

## Delete — do not maintain

Delete `src/runtime.rs` and its `pub mod runtime` declaration. Preserve the
built-in body, unknown-name result, CLI exit behavior and both integration tests
on the definitions owner. No compatibility export, adapter or duplicate lookup.

## Migration coverage

| Responsibility | Production consumers | Test consumers | Replacement/deletion | Exit check | Status |
| --- | --- | --- | --- | --- | --- |
| Skill lookup (this slice) | CLI `src/main.rs` | `tests/cli.rs` | Move `runtime::skill` to `definitions::skill`; delete runtime module | All targets compile; exact body and missing-name tests pass | Planned |
| Graph definitions | Outside fixture; callers unavailable | Outside fixture | Move to definitions owner; map and remove predecessor in full repository | Compile actual consumers and exercise Graph definition journey | Pending, outside selected journey |
| Native exports | Outside fixture; callers unavailable | Outside fixture | Switch exports to definitions owner without shim | Compile native consumers and exercise exported lookup | Pending, outside selected journey |

## Sequence and acceptance

1. Move lookup and all fixture imports, then `cargo check --all-targets`.
2. Strengthen the success test to assert the exact required body, so jointly
   changed library and CLI output cannot conceal a regression.
3. Run each named CLI integration test; format and lint all targets.
4. Time a whole-fixture build in a fresh local target directory. Record it
   separately from any cache probe. Neither measures Loopflow application build.

Demo: `target/fixture-build/debug/journey-ownership implement` prints the body;
`target/fixture-build/debug/journey-ownership missing` exits 2 with no output.
This slice is done when these behaviors and direct definitions ownership hold.
Full-target acceptance remains open for the two responsibilities above and
Loopflow's actual application build measurement.
