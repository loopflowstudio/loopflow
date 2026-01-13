# Ops Subcommand Split

## What exists
`lf` now exposes only run/inline/pipeline plus the shorthand task/pipeline resolution.
All non-task commands live under a single `ops` subcommand (`lf ops ...`).

## User intent (quotes)
> "I want to separate lf run and the other commanders."
> — user

> "I think we want two sepaate commands maybe"
> — user

> "lf is just the lf : and lf <command> functinoality"
> — user

> "Then we can have ... lander? that has all the other stuff"
> — user

> "should it all just be lf lander ?"
> — user

> "i think maybe just makes ops a single special sub command which can then be subdivided"
> — user

## CLI layout (implemented)
```python
# lf (minimal)
app = typer.Typer(name="lf", ...)
app.command()(run.run)         # lf run
app.command()(run.inline)      # lf :
app.command(name="pipeline")(run.pipeline)

ops = typer.Typer(name="ops", help="Management and maintenance commands.")
ops.add_typer(pr.app, name="pr")
ops.add_typer(maestro.app, name="maestro")

# Flat ops commands
ops.command()(meta.init)
ops.command()(meta.install)
ops.command()(meta.doctor)
ops.command()(meta.version)
ops.command()(status.status)
ops.command()(sessions.stop)
ops.command()(sessions.prune)
ops.command()(compare.compare)
ops.command()(land.land)

app.add_typer(ops, name="ops")

def main():
    # keep shorthand behavior: lf <task>, lf <pipeline>, lf :
    ...
```

## Decisions
- `meta` remains a module but its commands are flat under `lf ops` rather than `lf ops meta`.

## Constraints met
- Shorthand behavior preserved for `lf :`, `lf <task>`, and `lf <pipeline>`.
- Non-task commands moved under `lf ops` without backward compatibility.

## Verification
- `lf --help` lists only `ops` plus run/inline/pipeline.
- `lf ops --help` lists `pr`, `maestro`, `init`, `install`, `doctor`, `version`, `status`, `stop`, `prune`, `compare`, `land`.
- Commands documented in `README.md`, `docs/index.md`, and `docs/patterns.md`.

## Remaining work
- None.
