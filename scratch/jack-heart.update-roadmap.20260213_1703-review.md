# builtins: auto-generate HashMap registrations via build.rs

## What was implemented

A `build.rs` script that scans `src/engine/builtins/` subdirectories and generates `include!`-able Rust source files, each defining a `LazyLock<HashMap<&str, &str>>` for builtin steps, flows, directions, and ops prompts. The hand-written registrations in `builtins.rs` (~145 lines of manual `m.insert(...)` calls) are replaced by four `include!()` lines.

## Key choices

**Build-time codegen over proc macros.** `build.rs` is simpler, doesn't require a separate crate, and the output is inspectable in `target/`. Proc macros would be heavier for this use case.

**File stem as key, not path-relative key.** `steps/code/debug.md` registers as `"debug"`, not `"code/debug"`. This matches the existing API contract — callers look up steps by short name. Trade-off: name collisions across subdirectories would silently overwrite. Currently no collisions exist, and the sorted insertion makes the winner deterministic.

**Absolute paths in generated `include_str!`.** Generated source lives in `OUT_DIR`, so relative paths from there back to the source tree would be fragile. Absolute paths are reliable and canonical.

## How it fits together

`build.rs` runs at compile time, walks four subdirectories, and writes four `.rs` files to `OUT_DIR`. Each generated file defines one `static LazyLock<HashMap>`. `builtins.rs` pulls them in via `include!()`. The public API (`get_builtin_step`, `builtin_step_names`, etc.) is unchanged — callers don't know or care that registration is now automatic.

## Risks and bottlenecks

**Name collision.** If two files in different subdirectories of `steps/` share a stem (e.g., `code/review.md` and `plan/review.md`), one silently wins. Currently safe — `review.md` only exists in `plan/`. Worth a build-time assertion if the collection grows.

**Platform path separators.** The `replace('\\', "/")` in path stringification handles Windows backslashes in `include_str!` paths. Not a current concern (CI is macOS/Linux) but good defensive code.

## What's not included

- No changes to the public API surface — all existing functions remain with identical signatures
- No changes to `BUILTIN_CATEGORIES` in `discovery.rs` (the display-oriented list is separate and still hand-maintained)
- The removed `builtin_ops_prompt_names()` was dead code (no callers)

## Gate polish applied

- Replaced `unwrap()` with `expect("reason")` / `unwrap_or_else(|e| panic!(...))` per CLAUDE.md Rust style
- Ran `cargo fmt` (one formatting fix in `build.rs`)
- `cargo clippy -- -D warnings` passes clean
- All Rust tests pass (`cargo test --all`)
- Bonus: `update-roadmap.md` step (added in an earlier commit but never registered manually) is now automatically picked up
