"""Probe checkout identity using the production resolver's Git command."""

import subprocess
import tempfile
from pathlib import Path


def _git_path(cwd: Path, argument: str) -> Path:
    output = subprocess.check_output(
        ["git", "rev-parse", argument], cwd=cwd, text=True
    ).strip()
    return (cwd / output).resolve()


def _probe() -> None:
    checkout = Path(__file__).resolve().parent.parent
    main = _git_path(checkout, "--git-common-dir").parent
    assert checkout != main, "probe requires the supplied linked checkout"
    with tempfile.TemporaryDirectory(prefix="loopflow-checkout-probe-") as directory:
        alias = Path(directory) / "alias"
        alias.symlink_to(checkout, target_is_directory=True)
        for cwd in (checkout, checkout / "rust/loopflow/src", alias / "scratch"):
            assert _git_path(cwd, "--show-toplevel") == checkout
        assert _git_path(main, "--show-toplevel") == main
    print("PASS: root, subdirectory, and symlink cwd resolve to one checkout")
    print("PASS: existing main and linked checkouts resolve to distinct roots")
    print("LIMIT: no Task association, remote Home, or missing-path behavior tested")


if __name__ == "__main__":
    _probe()
