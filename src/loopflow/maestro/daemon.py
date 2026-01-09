"""Maestro daemon entry point."""

from pathlib import Path

from loopflow.maestro.service import Maestro


def main():
    """Run the maestro daemon."""
    lf_dir = Path.home() / ".lf"
    socket_path = lf_dir / "maestro.sock"
    state_path = lf_dir / "maestro.json"

    maestro = Maestro(socket_path, state_path)
    maestro.run()


if __name__ == "__main__":
    main()
