# 15232c: a flag missing its value says so

## Implementation

Both slices share the command relocation path and were built together. The
branch CLI exited 2 in all seven acceptance cases. It also named the missing
`--wave <WAVE>` for `task --wave --title x create` and `--after <AFTER>`
for `session history show id --after`. Clap prints a help hint without a
usage line. The test checks the owning command in the reordered argv.

`cargo test -p loopflow --bin lf reorder_args` passed all 16 tests. Gate
and CI own the remaining parser and navigation suites.

## Outcome

A flag works on either side of the command it belongs to. When it needs a
value and none was written, `lf` says that, wherever the flag sat:

```
$ lf open --repo
error: a value is required for '--repo <NAME_OR_PATH>' but none was supplied
$ echo $?
2
```

Today it prints `Error: resolve repository /Users/jack/src/open`, because the
command name became the flag's value.

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
`reorder_args` moves them ahead of the command, and moves a flag written
beside the wrong command on the path to its owner. "Which token is this flag's
value" is answered three times in that file, each differently from Clap:

- `push_flag` (line 123) takes the next token whenever one exists, including
  `--tab` and `--`, and moves the flag alone when none does. Its callers then
  put the bare flag in front of the command, the skill name or a child
  command, which Clap reads as the value.
- `first_target_index` and `selected_command_path` skip the next token with
  their own `index + 1 < len` checks, so `task --wave --title x create` loses
  its command path: `--title` is skipped as `--wave`'s value and `x` is not a
  subcommand.

## Design

Two pieces. Both replace something; nothing is added beside the old code.

### 1. One answer to "where is this flag's value"

`FlagTables` holds one map from flag name to what it takes, in place of the
`value` and `boolean` sets:

```rust
enum Takes { Nothing, OptionalValue, Value }
```

filled from the Clap tree: `Nothing` when the action takes no values,
`OptionalValue` when `get_num_args()` allows zero (`session list --primary`,
`lf/mod.rs:1701`, the only one today), `Value` otherwise.

One function reads a flag at a position and answers:

```rust
enum FlagValue { None, Inline, Next, Missing }
```

- `Inline`: `--repo=x`.
- `Next`: the following token exists and is a value by Clap's rule: it does
  not start with `-`, or it is a lone `-`.
- `Missing`: the flag is `Takes::Value` and the next token is absent, `--`, or
  starts with `-`. No flag in the tree sets `allow_hyphen_values` or
  `allow_negative_numbers`, so there is no per-flag exception.
- `None`: `Takes::Nothing`, or `OptionalValue` with no value following.

`push_flag` goes. `first_target_index`, `selected_command_path`,
`reorder_command_args` and the skill branch all call this function; none
keeps an `index + 1 < len` check of its own.

### 2. A relocated flag that is missing its value is the whole argv

When `reorder_args` is about to move a flag and its value is `Missing`, it
returns the shortest argv that reaches that flag at its owner, and nothing
else:

| written | returned |
| --- | --- |
| `lf open --repo` | `lf --repo` |
| `lf -a codex implement: fix it --repo --tab` | `lf --repo` |
| `lf task --wave --title x create` | `lf task create --wave` |

`lf`, then the command words through the flag's owner (none for a root flag),
then the flag as written. Clap names the owner's flag and prints its help hint.
Everything else is dropped because the parse cannot succeed and nothing reads it: every caller hands the result to `Cli::try_parse_from`
(`lf.rs` 1624, 1765, 1826; 1786 through `ssh::parse_remote_command`, which
returns Clap's error before anything is sent to another Machine). The returned
argv does not depend on what else was moved, so it is the same from both
branches and stable under a second `reorder_args` (the `--machine` path
reorders twice).

A flag that is not being moved is left to Clap as written, missing value or
not.

### Choices made for review

- **Shortest argv, not "everything up to the flag".** The first draft kept
  leading args and the globals moved so far. Dropped: the error is the same,
  and one constructor with no accumulated state is easier to test by equality.
  Cost: `lf --bogus open --repo` reports the missing value, not `--bogus`.
- **One `Takes` map instead of two sets plus a "requires a value" set.** Three
  states are the real thing; `--primary` stops being treated as always taking
  the next token.

### Passed over

- **Leave the valueless flag where it was written.** For `open`, Clap answers
  `unexpected argument '--repo'`, the wrong problem. For a skill it is worse:
  the flag becomes a word of the prompt and an agent starts.
- **`reorder_args` returns `Result<_, clap::Error>`.** Clap's missing-value
  error is built by parsing an argv that ends at the flag, which is the
  returned argv anyway; four callers would gain an error path for nothing.
- **Make root flags `global = true` and delete the reordering.** Local flags
  win collisions today (`commit -m`), and a skill is an external subcommand
  that swallows everything after its name. A separate outcome; see below.

## Slices

1. **The value function and root flags.** `Takes`, `FlagValue`, the one
   function, all four callers on it, `push_flag` deleted, and the short argv
   for root flags after a command and after a skill. Tests for `open --repo`,
   `implement --repo`, `open --repo --tab`. Then the built binary on
   `lf open --repo` and `lf open --agent`: this shows whether Clap prints the
   missing-value error for `lf --repo` reached this way, and that no agent
   starts.
2. **Flags owned elsewhere on the path.** `task --wave --title x create`
   returns `lf task create --wave`; check the diagnostic names
   `--wave <WAVE>`. This depends on slice 1's function in
   `selected_command_path`, or `create` is never found.

Done when the acceptance loop prints seven missing-value errors and the
existing `reorder_args_*` tests pass unchanged.

## Tests

In `lf.rs` `mod tests`. Each asserts the argv `reorder_args` returns, then
that `Cli::try_parse_from` fails with the missing-value kind and text naming
the flag. Read the kind Clap returns from the first failing run; don't guess
it.

- command then valueless root flag: `open --repo`, `open --task`,
  `open --agent`, `steer session_1 m --repo`, `task list --repo`
- skill then valueless root flag: `implement --repo`, `implement: fix it -a`
- flag moved down to its owner: `task --wave --title x create`
- flag moved up to a parent, written last after the child: pick a value flag a
  parent with subcommands owns directly, from `arg_tables()`; if the tree has
  none, the downward case is the only one that exists
- followed by a flag or `--`: `open --repo --tab`, `open --repo -c`,
  `implement --repo --`
- unchanged: `open --repo=x`, `open --repo x`, `open --repo -` (lone `-` is a
  value), `debug -a codex`, `skill debug -- --wave literal`, and
  `reorder_args` applied twice gives the same argv

Run: `uv run python scripts/test.py --reuse-passing --rust-filter 'binary(lf) & test(reorder_args)'`.

`rust/loopflow/src/lf/mod.rs:3658` (`steer session_1 m --repo` is an error)
parses without `reorder_args`, so it passes for the wrong reason; the `steer`
case above covers the real path. Leave the old line: it still pins that
`steer` has no `--repo`.

## Acceptance check at gate

```bash
cargo build -p loopflow --bin lf
LF=$PWD/target/debug/lf
cd /tmp && for a in "open --repo" "implement --repo" "task list --repo" \
  "open --task" "open --agent" "open --repo --tab" "open --repo --"; do
  $LF ${=a} </dev/null; echo "exit $?"; done
```

Every line: `error: a value is required for '--<flag> <…>' but none was
supplied`, exit 2, and no agent starts.

## Not this Task

- `lf task --repo list`: `list` follows the flag as written, so it is the
  value, as Clap itself reads it.
- **Prose after `skill:` is scanned for flags.** `lf implement: fix the --repo
  flag` sets the repository to `flag` and sends the prompt `fix the`; `-c`
  after a prompt is lifted on purpose (`reorder_args_skill_with_args`). After
  this Task a trailing `--repo` there is a missing-value error instead of a
  shortened prompt. Deciding which words of a prompt are flags is a product
  choice with its own Task; quoting or `--` passes them literally today.
- **Attached short values.** `lf implement -acodex` is not recognized by
  `lf.rs` (it becomes a prompt word); `navigation::takes_separate_value`
  does know the form.
- **Three scanners, two flag tables.** `navigation::normalize_args`,
  `navigation::machine_invocation` and `reorder_args` each walk the argv with
  their own reading of the Clap tree. The navigation pair copy tokens in order
  and never put a flag ahead of a word, so they cannot cause this failure, and
  they keep `takes_separate_value`. One pass that resolves the command path,
  each flag's owner and its placement would delete `CommandArgTables`; so
  would Clap globals plus a small skill-only lift. Either is its own Task.
