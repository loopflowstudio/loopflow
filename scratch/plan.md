# 15232c: a root flag missing its value says so

## Outcome

`lf open --repo` prints Clap's own error and exits 2:

```
error: a value is required for '--repo <NAME_OR_PATH>' but none was supplied
```

Today it prints `Error: resolve repository /Users/jack/src/open`, because the
command name became the flag's value. The same holds for every value-taking
flag that `reorder_args` relocates: after a command, after a skill, and for a
parent command's flag written after its child.

## What happens today (installed `lf`, run from `/tmp/lfprobe`, 2026-11-21)

| argv | result |
| --- | --- |
| `lf open --repo` | `Error: resolve repository /Users/jack/src/open` |
| `lf implement --repo` | `Error: resolve repository /Users/jack/src/implement` |
| `lf task list --repo` | `Error: resolve repository /Users/jack/src/task` |
| `lf open --task` | `Error: unplaced Task needs a repository` (Task `open`) |
| `lf open --agent` | agent `open`, no command left: goes on to launch the default agent |
| `lf open --repo --tab` | `error: unexpected argument '--tab' found`, tip `--task` |
| `lf open --repo --` | missing-value error, by accident: `--` was moved as the value |
| `lf --repo` | missing-value error (never reordered) |

`lf open --agent` is the worst one: a typo starts an agent.

## Cause

`rust/loopflow/src/bin/lf.rs`. Root flags are not Clap globals, so
`reorder_args` moves them ahead of the command. `push_flag` (line 123) takes
the next token as the value whenever one exists, and moves the flag alone when
none does. Both callers then place the flag before a word it never preceded:

- `reorder_command_args` puts `moved_globals` before the command, and
  `moved_locals` between a parent and its child.
- the skill branch of `reorder_args` puts the flag before the skill name.

`push_flag` also takes `--tab` or `--` as a value, which Clap never would.

## Design (one mechanism, chosen for review)

**A relocated flag that lacks its value ends the argv.** One helper answers
"where is this flag's value": inline (`--repo=x`), the next token, or missing.
Missing means Clap's rule: no next token, or the next token starts with `-`
and is longer than `-` (so `--`, `--tab`, `-c`; a lone `-` is a value). No flag
in the tree sets `allow_hyphen_values` or `allow_negative_numbers`
(`grep` over `rust/loopflow/src`, `rust/loopflow-execution/src`), so the rule
needs no per-flag exception.

When a flag being moved is missing its value, `reorder_args` returns
everything that precedes the flag at its destination, then the flag, and
nothing after:

- root flag: `["lf", <leading args>, <globals moved so far>, "--repo"]`
- flag owned by another command on the path: the same, plus the command path
  through its owner: `["lf", "task", "create", "--wave"]` for
  `lf task --wave --title x create` (`--wave` belongs to `create`; see
  `reorder_args_moves_flags_to_nested_owners`)

Clap then reports the missing value itself, with the owner's usage line. The
rest of the argv is dropped because the parse cannot succeed and nothing reads
it: every caller hands the result to `Cli::try_parse_from` (`lf.rs` 1624,
1765, 1826; 1786 through `ssh::parse_remote_command`, which returns Clap's
error before anything is sent to another Machine).

`push_flag` goes; the helper replaces it. `first_target_index` and
`selected_command_path` use the same helper in place of their own
`index + 1 < len` checks, so `lf.rs` has one definition of a flag's value.

A flag whose value is optional is never "missing". `FlagTables.value` is
filled from `takes_values()`, which is true for `session list --primary`
(`num_args = 0..=1`, `lf/mod.rs:1701`), the only such flag today. It is never
relocated (its owner has no child), but the table should record "requires a
value" (`get_num_args()` minimum ≥ 1) so the truncation can't fire on one
later.

### Passed over

- **Leave the valueless flag where it was written.** Clap answers
  `unexpected argument '--repo'` for `open`, which names the wrong problem.
- **`reorder_args` returns `Result<_, clap::Error>`.** Clap's missing-value
  error can only be built by parsing an argv that ends at the flag, which is
  the truncated argv anyway; four callers would gain an error path for
  nothing.
- **Make root flags `global = true` and delete the reordering.** Local flags
  win collisions today (`commit -m`), and skills are an external subcommand
  that swallows everything after the name. Larger than this Task.

## Acceptance check at gate

```bash
cargo build -p loopflow --bin lf
cd /tmp && for a in "open --repo" "implement --repo" "task list --repo" \
  "open --task" "open --agent" "open --repo --tab" "open --repo --"; do
  target/debug/lf ${=a} </dev/null; echo "exit $?"; done
```

Every line: `error: a value is required for '--<flag> <…>' but none was
supplied`, exit 2, and no agent starts. (`target/debug/lf` by its full path
from the checkout.)

Unit tests, in `lf.rs` `mod tests`, each through `reorder_args` then
`Cli::try_parse_from`, asserting the error's kind and that its text names the
flag. Read the kind Clap returns for a missing value from the first failing
run; don't guess it.

- command then valueless root flag: `open --repo`, `open --task`,
  `steer session_1 m --repo`, `task list --repo`
- skill then valueless root flag: `implement --repo`, `implement: fix it -a`
- flag moved down to its owner: `task --wave --title x create`
- flag moved up to a parent, written last after the child: pick a value flag a
  parent with subcommands owns directly, from `arg_tables()`; if the tree has
  none, the downward case is the only one that exists
- flag followed by a flag or `--`: `open --repo --tab`, `open --repo -c`,
  `implement --repo --`
- unchanged: `open --repo=x`, `open --repo x`, `debug -a codex`,
  `skill debug -- --wave literal`, and `reorder_args` applied twice gives the
  same argv (the `--machine` path reorders, then the local path reorders again)

Run: `uv run python scripts/test.py --reuse-passing --rust-filter 'binary(lf) & test(reorder_args)'`.

The existing assertion in `rust/loopflow/src/lf/mod.rs:3658`
(`steer session_1 m --repo` is an error) parses without `reorder_args`, so it
passes for the wrong reason; the new `steer` case in `lf.rs` covers the real
path. Leave the old line: it still pins that `steer` has no `--repo`.

## Exclusions

- `lf task --repo list`: `list` follows the flag as written, so it is the
  value, as Clap itself would read it.
- `lf : fix the --repo flag`: reordering already lifts root flags out of an
  inline prompt (`-c` today). After this change a trailing `--repo` there is a
  missing-value error instead of a lost prompt; quoting or `--` is the way to
  pass it literally. Not changing what gets lifted.
- `navigation::normalize_args` and `machine_invocation`
  (`rust/loopflow/src/lf/navigation.rs:157,234`) keep their own
  `takes_separate_value`. They copy tokens in order and never move a flag
  ahead of a word, so they cannot cause this.

## What goes

- `push_flag`.
- The two hand-written `index + 1 < len` value checks in `lf.rs`.

## Slices

1. **Root flag after a command or skill.** The helper, the truncation in both
   branches, tests for `open --repo` and `implement --repo`, then the built
   binary on `lf open --repo` and `lf open --agent`. This shows whether Clap
   prints the missing-value error for a truncated argv in every position
   (leading args present, flags already moved).
2. **Flags owned elsewhere on the path** through `moved_locals`, with
   `task --wave --title x create`; check the usage line names `lf task create`.
3. **Followed by a flag or `--`**, the "requires a value" table, the shared
   helper in `first_target_index` and `selected_command_path`, idempotence.

Done when the acceptance loop prints seven missing-value errors and the
existing `reorder_args_*` tests pass unchanged.
