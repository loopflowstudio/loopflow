Read when the change touches how `lf` parses or reorders its arguments (`reorder_args`, global flags after a command or skill).

# Argument parsing

Parsing errors exit before any agent, store or server work, so drive the
branch binary straight from a throwaway repository with an isolated home:

```zsh
B=$PWD/target/debug/lf      # built with LOOPFLOW_MIGRATION_ORDER in a shallow checkout
cd "$(mktemp -d)" && git init -q
env -i PATH=/usr/bin:/bin HOME=$PWD/home LF_HOME=$PWD/lfhome $B open --repo </dev/null
```

`lf` from `PATH` may be a shell function; compare against
`/Applications/Loopflow.app/Contents/MacOS/lf` for the released behavior.

To show that a skill launch parsed without starting a provider, pass
`-a nope`: a parsed command fails at agent resolution with
"unsupported agent harness 'nope'" (exit 1), while a parse failure exits 2
with Clap's "a value is required for '--flag <VALUE>'". `lf task create
--title … --json` and `lf task list --json --repo <path>` check value flags
against real rows in the private home.
