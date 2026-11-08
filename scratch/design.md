# LOO-804: Served launch results

The Task brief calls for reopening the Machine registry while polling and
bounding unknown liveness so a missing report cannot leave the caller waiting
indefinitely. Returning `outcome_unknown` must not imply the work stopped or
authorize an automatic retry.

`docs/troubleshooting.md` (“A launch reports `outcome_unknown`”) describes the
intended behavior and recovery guidance. No timeout duration is promised there.

The poll reopens the registry each time. Five seconds of continuously unknown
liveness returns `outcome_unknown`; a live observation resets that grace.
Reports win over exit or expiry, including a final read after either observation.
The wait does not stop or retry the owner. No new schema or wire shape.

Reports deserialize directly into the typed result; malformed reports still
return `outcome_unknown`. No implementation work remains identified.

Concept review: no design change or open product choice. The launch reply says
what the caller knows; Task and Process records say what the work is doing.
Keep that distinction without adding a persisted launch state, retry action or
timeout setting. Existing operating guidance already requires inspecting work
and leaving live Graphs running.

Counterexamples for QA: a report racing owner exit or unknown expiry must win;
a registry recovery must be observed; a late success after `outcome_unknown`
must remain discoverable through the affected work, without relaunching it.
A known-live owner may wait longer than the unknown grace. This is not an
overall command deadline. Show the served-client recovery path next; the
concept review does not replace that check.

Remaining: review, sync with current `origin/main` and gate, then publication,
CI repair if needed, merge and completion. Real served-client QA is recorded in
`scratch/qa.html`: registry loss returned uncertainty in 5.05 seconds for a Task;
restoring it and resuming the original command produced exactly one completed
Graph, found through `task status` and `task runs`. Direct Graphs also recovered,
and a restored registry kept a live launch waiting beyond the grace. No runtime
repair was needed. Troubleshooting now includes finished direct Graph history.
Gate reaches
`test(engine::launch::tests::) | binary(served_reads) | test(documented_commands::)`
plus architecture and website checks from `scripts/test.py --list`; the served
suite covers launch refusals through the client API. Formatting and clippy
remain required before the next code commit. No product decision is open.

`cargo build -p loopflow --bin lf`: passed; branch CLI used for real socket QA.
`uv run python scratch/qa_task_launch.py`: normal and late Task scenarios passed;
all private server/child PIDs exited after cleanup. `scratch/qa.html` holds limits.

`cargo test -p loopflow --lib engine::launch::tests::` (inherited `LF_*` unset,
stdin closed, after compress): 6 passed; linker warned that the test binary's `__eh_frame`
exceeds 16 MB. `cargo fmt` and `git diff --check` passed.

`lf open --host desktop --file docs/troubleshooting.md` could not present the
file: “has no workspace in this Desktop window; nothing changed.”
