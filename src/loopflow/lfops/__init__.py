"""lfops: Loopflow operations CLI."""


def main() -> None:
    """Entry point for lfops command."""
    import sys

    import typer

    typer.echo(
        "Warning: 'lfops' is deprecated. Use 'lf ops' instead.\n"
        f"  Example: lf ops {' '.join(sys.argv[1:])}\n",
        err=True,
    )

    from loopflow.lfops.commands import main as _main

    _main()


def get_app():
    """Get the Typer app (lazy import to avoid circular imports)."""
    from loopflow.lfops.commands import app

    return app


__all__ = ["main", "get_app"]
