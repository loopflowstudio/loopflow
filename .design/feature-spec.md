# Ops Subcommand Split

## What to build
Keep `lf` as the single CLI, but move all non-task commands under a single `ops` subcommand (e.g., `lf ops pr`, `lf ops land`, `lf ops status`).

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

## Data structures
```python
# No new data structures expected; rewire existing Typer apps/commands.
```

## CLI layout (sketch)
```python
# lf (minimal)
app = typer.Typer(name="lf", ...)
app.command()(run.run)         # lf run
app.command()(run.inline)      # lf :
app.command(name="pipeline")(run.pipeline)

ops = typer.Typer(name="ops", help="Management and maintenance commands.")
ops.add_typer(pr.app, name="pr")
ops.add_typer(meta.app, name="meta")
ops.add_typer(maestro.app, name="maestro")
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

## APIs
```python
# Add ops subcommand wiring in src/loopflow/cli/__init__.py
# Possibly introduce a new module: src/loopflow/cli/ops.py
```

## Constraints
- Preserve existing `lf` shorthand behavior for `lf :`, `lf <task>`, and `lf <pipeline>`.
- Keep Typer conventions and subcommand names unchanged; only relocate them.
- Avoid breaking existing task/pipeline resolution logic in `lf` main.
- No backward compatibility for old `lf <subcommand>` names; move everything immediately.
- `lf ops` is the management namespace.

## Done when
- `lf` only exposes run/inline/pipeline + shorthand behavior for tasks/pipelines.
- `lf ops` exposes pr/meta/maestro/status/sessions/compare/land.
- `lf --help` lists `ops` as a single subcommand.
- `lf ops --help` shows management commands.
- Existing commands still function under their new namespace.

## Open questions
- Where should help text and docs be updated (README, docs/index.md, docs/patterns.md)?
