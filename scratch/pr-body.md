## Try it!

```bash
cargo build
lf goal s3 --once
```

`lf goal s3` resolves the builtin goal named `s3` — the VSM control charter — through the existing goal-loader precedence (`.lf/goals/` → `wave/<name>/GOAL.md` → builtin). In this repo `s3` has no wave dir, so it renders the charter with empty roadmap/memory, launches, sees no safe move, and halts cleanly.

## Intent

Ship the five Viable System Model system charters as **builtin goals** `s1`…`s5`. Each is a short, generic, self-correcting compass a looping agent can run directly with `lf goal s1`…`lf goal s5`.

## Assumptions

Goal-loader precedence remains the right abstraction: repo overrides and wave-local `GOAL.md` files can shadow builtin goals by name. A standing system loop gets memory or roadmap context by adding a matching `wave/s1`…`wave/s5` directory later.

## Key decisions

**No VSM-specific code, anywhere.** `load_goal` already falls back to builtin goals by name, and `resolve_wave_name` doesn't require a `wave/<name>/` dir — so `lf goal s3` reaches the builtin `s3` charter through the generic path with zero new plumbing. The five charters are plain markdown under `builtins/govern/goal/`, auto-registered by `build.rs`.

**A wave overrides a builtin by name.** Drop a `wave/s3/GOAL.md` to override the `s3` charter; add a `wave/s3/MEMORY.md` and it layers in as context. Precedence is unchanged: repo override → wave `GOAL.md` → builtin.

**No `lf goal` changes.** The command is byte-identical to `main`. An earlier iteration added a `--system s1..s5` flag and a `resolve_system_goal` map; that special-casing was redundant with the loader and has been removed.

## Not included

No wave directories for `s1`…`s5` (they resolve as builtins today; add a dir when a system needs standing memory). No changes to the `govern-*` flows — they remain each system's hand. No scheduler or UI for always-on VSM loops.
