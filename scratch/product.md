# Explicit radio pub/sub

> "i think lf sub should be a radio subcommand"
>
> "i think i prefer the explicit"

## What to build

Make `radio` the agent-bus namespace with explicit `pub` and `sub` operations,
and remove the old top-level publish/subscription spellings.

```text
lf radio pub [TEXT] [--channel NAME | --parent] [--from NAME]
lf radio sub [CHANNEL] [--json]
```

Bare `lf radio` prints its subcommand help. `lf sub` is no longer a command.
There is no compatibility alias: every builtin prompt, example, and caller
moves to the one grammar in the same change.

## The demo

In one terminal:

```bash
lf radio sub product
```

In another:

```bash
lf radio pub -c product --from demo "button audit finished"
```

The subscriber prints `[product] demo: button audit finished`. `lf radio`
shows `pub` and `sub`; `lf sub` and `lf radio "text"` fail at parsing.

## Data structures

```rust
enum Commands {
    Radio {
        #[command(subcommand)]
        command: RadioCommand,
    },
}

enum RadioCommand {
    Pub {
        text: Vec<String>,
        channel: Option<String>,
        parent: bool,
        from: Option<String>,
    },
    Sub {
        channel: Option<String>,
        json: bool,
    },
}
```

The store bus, channel-prefix matching, cursor behavior, byline rules, and
publish/subscribe implementations do not change. This is command ownership,
not a transport rewrite.

## Key functions

```rust
radio::run_pub(text, channel, parent, from) -> Result<()>
sub::run(channel, json) -> Result<()>
```

Dispatch through `Commands::Radio { command }`. The wave exec door continues
to authorize radio as one capability; both operations are bus-only and never
gain access to chat, listener lifecycle, or arbitrary execution.

## Constraints

- `lf chat` remains the human thread. `lf radio pub/sub` remains the ephemeral
  agent bus. The namespace must not fuse their transports or retention rules.
- Publishing still reads stdin when TEXT is omitted, so headless reports and
  heredocs remain usable.
- Subscribing still exits cleanly when no ambient channel or store resolves.
- Update builtins atomically: detached hands currently receive literal
  `lf radio` and `lf sub` instructions.
- Do not keep top-level aliases. The repository does not preserve internal CLI
  compatibility unless a migration is explicitly required.
- Keep implementation ownership simple. Nesting the CLI does not require a
  new bus abstraction or moving stable store code.

## Done when

- CLI parser tests prove both new forms, bare-radio help, and rejection of the
  two old forms.
- Existing publish behavior tests pass through `radio pub`; existing prefix,
  NDJSON, and Ctrl-C subscription behavior passes through `radio sub`.
- `rg 'lf (radio(?! (pub|sub))|sub)'` over user docs and builtin guidance finds
  no stale executable examples (excluding historical prose where intentional).
- `cargo fmt --all -- --check` passes.
- Focused `lf` command and bus tests pass.
- `cargo clippy -p loopflow --all-targets -- -D warnings` passes.
