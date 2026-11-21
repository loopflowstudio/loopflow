# 15232c Fix missing values on reordered flags

The Task brief says, “Fix `reorder_args` so valueless value-taking flags cannot hijack the following command, subcommand, or skill name as their value and instead surface Clap's missing-value error. Add regression tests in `rust/loopflow/src/bin/lf.rs` covering `reorder_args` + `Cli::try_parse_from`.”

## Implemented

Root and nested flags share the same rule for finding a separate value in
`rust/loopflow/src/bin/lf.rs`. When a flag needs moving and its required
value is missing, the rewrite ends at that flag under its owning command.
Clap can then report the error before it treats a command or skill name as
the value. The regression test checks the owning command in the rewritten
arguments, confirms a second rewrite leaves them intact, and checks the
diagnostic and exit status. It covers flags moved to a parent or a child.

The installed CLI observed on 2026-11-21 resolved
`/Users/jack/src/open` for `lf open --repo`. The branch CLI instead exited
2 in all seven acceptance cases below. Additional branch runs named
`--wave <WAVE>` for `task --wave --title x create` and `--after <AFTER>`
for `session history show id --after`. The original plan expected the
owner's usage line. Clap names the flag and ends with
`For more information, try '--help'.`

All 16 tests passed in `cargo test -p loopflow --bin lf reorder_args`
after realign collapsed the two cases that consume no separate token and
added the diagnostic flag assertion. This shallow checkout needed
`LOOPFLOW_MIGRATION_ORDER=/tmp/lf-15232c-migration-order.txt` for the build.
The recorded base still matched `origin/main` at realign.

## Next

Compress rebuilt the CLI and replayed all seven cases below from /tmp. Each
named its missing flag and exited 2. All 10 grouped reorder_args tests passed.
CI owns the broader parser and navigation suites.

```zsh
cargo build -p loopflow --bin lf
repo_cli=$PWD/target/debug/lf
cd /tmp
for argv in "open --repo" "implement --repo" "task list --repo" \
  "open --task" "open --agent" "open --repo --tab" "open --repo --"; do
  "$repo_cli" ${=argv} </dev/null
  print "exit $?"
done
```

Each invocation must name its missing flag value and exit 2 without launching
an agent.

`lf : fix the --repo flag` still scans prompt words for flags. Attached short
values such as `-acodex` and consolidation with the scanners in
`rust/loopflow/src/lf/navigation.rs` are separate work.

## Concept review of the built change

Reviewed against base `12b66c3e17` on 2026-11-21; nothing to change.
`FlagValue::separate_value` is the only reading of "this flag's value" in
`lf.rs`, and it matches Clap. `scratch/concept-review.html` holds the run
results.

A word written directly after a value-taking flag stays its value even when
it names a subcommand, as Clap reads it and as the base did. On the branch,
`lf task --wave create` printed `lf task` usage and exited 2,
`lf session history --after show id` printed `invalid value 'show' for
'--after <AFTER>'`, and `lf session --primary list` printed `unexpected
argument '--primary'` with the tip `'list --primary' exists`. Letting
subcommand names win was passed over: a Wave named `list` would then need
`--wave=list`.
