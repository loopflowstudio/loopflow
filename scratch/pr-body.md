## Try it!

```bash
cargo build
cargo test -p loopflow
lf goal root --system s3 --once
```

`--system s3` runs the builtin `govern-control` charter against `root`'s wave context. In this repo today, `root` has no roadmap handle or metrics, so the one-shot loop launches, sees there is no safe move to make, and halts cleanly.

For direct verification:

```bash
cargo test -p loopflow build_goal_message_can_render_system_goal_against_wave_context
cargo test -p loopflow build_goal_message_system_goal_ignores_repo_goal_override
```

## Intent

Promote the five VSM systems from only govern flows into builtin goal charters that can run against any chord. `lf goal <wave> --system s1..s5` gives a looping agent a generic system-level compass while preserving the selected wave's roadmap, memory, metrics, available flows, and in-flight work.

## Assumptions

System names are the lowercase shorthand values `s1` through `s5`. The shipped charter wording is authoritative, so `--system` loads embedded builtin goal text directly instead of allowing repo-local goal overrides.

## Key decisions

The feature reuses the existing goal renderer and launch path instead of adding a new runtime type. `--system` changes only the goal body; wave context still comes from the named wave.

The five builtin goal filenames match the existing govern flow names: `govern-operations`, `govern-coordination`, `govern-control`, `govern-intelligence`, and `govern-identity`.

## Not included

No changes to the existing `govern-*` flows. No standing scheduler or UI for always-on VSM agents. No aliases beyond `s1` through `s5`.
