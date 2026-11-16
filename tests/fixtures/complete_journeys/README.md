# Complete-journey scenarios

These are small authored demonstrations for the large-change skills, derived
from ownership and client-state migrations. They exercise executable fixture
behavior, not Loopflow's production migration or native views. They are not an
independent agent trial or a prompting-quality benchmark.

Run from the repository root:

```bash
uv run python tests/fixtures/complete_journeys/run.py
```

The replay prints source hashes and command timings, constructs the initial
ownership fixture, catches an unmigrated test import, and finishes that cut. It
also checks that a reducer-only Complete fails before running the completed
consumer journey. All builds and mutations stay in a temporary directory; no
agent is launched. Timings cover these small fixture commands, not the product.

For a new agent trial, copy the fixtures into a disposable repository with no
remote, give the exact scenario input below and the skill sources being tested,
and retain the submitted prompt, source revision, patch, commands and results.
Do not run either scenario against a live Task. The scenarios grant no authority
to publish, merge or change the motivating Tasks.

## Ownership input

> Move skill lookup from the runtime owner to the definitions owner. Start with
> the CLI lookup journey, keeping its body and missing-name behavior. Compile
> production and test consumers early. Keep other responsibilities visible in
> the migration table. No re-export, adapter or duplicate lookup implementation.
> Measure the whole fixture build separately from any cache probe; neither is
> a measurement of Loopflow's application build.

The starting shape is `src/runtime.rs` containing `skill`, `lib.rs` declaring
`runtime`, and both `main.rs` and `tests/cli.rs` importing `runtime::skill`.
To reconstruct it in a disposable copy, rename `definitions.rs` to `runtime.rs`
and replace `definitions` with `runtime` in the other three source files.
The completed representative cut is the checked-in fixture. One implementation
serves the CLI and the library test consumer. The CLI's observable result and
missing-name exit code survive the owner move.

| Responsibility | Production consumers | Test consumers | Replacement/deletion | Exit | Status |
| --- | --- | --- | --- | --- | --- |
| Skill lookup | CLI, library | `tests/cli.rs` | `runtime::skill` → `definitions::skill`; remove runtime module | Known body and unknown exit 2 | Complete in fixture |
| Graph definitions | Graph loader | Graph parsing tests | Outside this fixture | Load real graphs | Pending in full migration |
| Native exports | Skill export | Export tests | Outside this fixture | Resolve exported methods | Pending in full migration |

A test consumer found after moving the production import updates the lookup
row; it is not a reason to declare a second migration or rebuild the old owner.

## State input

> Complete must persist the Task change, remove it from Desktop and mobile
> lists and select the next item. Check stale reply, restart and refusal.
> A reducer-only change is incomplete. Keep Create and Reopen pending. Do not
> claim native click-to-paint timing from a headless consumer fixture.

`state.py` has two list consumers over one persisted record. Its public
`TaskList.complete` action is the entry point; tests inspect both consumers and
reopen the saved file. This is a synchronous headless model. It does not model
transport loss, concurrent writers, process crashes during file writes or UI
rendering. Those remain production checks, not claims made by this fixture.

| Responsibility | Consumers | Replacement/deletion | Exit | Status |
| --- | --- | --- | --- | --- |
| Complete | Both list consumers, persisted store | Shared authoritative response replaces per-list removal decisions | Removal, next selection, stale reply, restart, refusal, empty list | Complete in fixture |
| Create and Reopen | Both lists | Outside this fixture | Full action lifecycle | Pending in full migration |
| Native responsiveness | Desktop/mobile views | No timing substitute | Real interaction measurement | Unmeasured |

## Review and continuation inputs

Use each independently with the working design and observed scenario results:

| Input | Expected assessment |
| --- | --- |
| “Only an internal helper name changed; the interaction, owners, persistence and recovery are unchanged. No open choice or regression.” | Brief unchanged review; no new page or redesign. This does not finish pending migration rows. |
| “The test consumer still imports runtime::skill; production imports definitions::skill.” | Add the missed consumer to the existing row, repair that cut, compile both callers. |
| “The inventory is unchanged for a second pass. No behavior closed, no new finding, no changed approach.” | No identical iteration; report what must change using the caller's Blocked protocol. |
| “Every required row and this step's checks is complete; no open findings. Publishing belongs to a later step.” | Advance this step without publishing or inventing more implementation. |

These review inputs are walkthrough cases until a separate agent run is
recorded. Executing the fixtures proves their behavior, not a model's choice of
review outcome. A useful first journey leaves the pending rows pending.
