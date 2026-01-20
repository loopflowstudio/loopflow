"""lfops: Loopflow operations CLI."""


def main() -> None:
    """Entry point for lfops command."""
    from loopflow.lfops.commands import main as _main

    _main()


def get_app():
    """Get the Typer app (lazy import to avoid circular imports)."""
    from loopflow.lfops.commands import app

    return app


__all__ = ["main", "get_app"]
