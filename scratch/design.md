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

Delete — do not maintain: the one-time registry connection, `ended` boolean,
and intermediate JSON value in the report reader. These cuts are complete;
there are no remaining deletion targets. Reports deserialize directly into
the typed result; malformed reports still return `outcome_unknown`.

Remaining: review, sync, QA and gate. Gate reaches
`test(engine::launch::tests::) | binary(served_reads) | test(documented_commands::)`
plus architecture and website checks from `scripts/test.py --list`; the served
suite covers launch refusals through the client API. Clippy belongs to gate
before committing.

`cargo test -p loopflow --lib engine::launch::tests::` (inherited `LF_*` unset,
stdin closed, after compress): 6 passed; linker warned that the test binary's `__eh_frame`
exceeds 16 MB. `cargo fmt` and `git diff --check` passed.

`lf open --host desktop --file docs/troubleshooting.md` could not present the
file: “has no workspace in this Desktop window; nothing changed.”
