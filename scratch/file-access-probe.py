"""Probe same-content symlink changes against the existing save restriction."""

import errno
import hashlib
import os
import tempfile
from pathlib import Path


def _revision(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _probe() -> None:
    with tempfile.TemporaryDirectory(prefix="loopflow-file-access-probe-") as directory:
        root = Path(directory)
        target = root / "target.txt"
        document = root / "document.txt"
        target.write_text("retained content\n")
        document.write_bytes(target.read_bytes())
        before = _revision(document)
        document.unlink()
        document.symlink_to(target.name)
        assert document.resolve().is_relative_to(root.resolve())
        assert _revision(document) == before
        try:
            descriptor = os.open(document, os.O_RDONLY | os.O_NOFOLLOW)
        except OSError as error:
            assert error.errno == errno.ELOOP, error
        else:
            os.close(descriptor)
            raise AssertionError("O_NOFOLLOW unexpectedly opened a symlink")

        real_directory = root / "real"
        real_directory.mkdir()
        (real_directory / "nested.txt").write_bytes(target.read_bytes())
        alias = root / "alias"
        alias.symlink_to(real_directory.name, target_is_directory=True)
        nested = alias / "nested.txt"
        assert not nested.is_symlink()
        assert nested.resolve().is_relative_to(root.resolve())
        assert _revision(nested) == before

    print("PASS: replacing a regular file with an internal symlink preserves its revision")
    print("PASS: O_NOFOLLOW rejects that same readable content")
    print("PASS: a regular leaf can traverse a symlink parent; leaf metadata is insufficient")
    print("LIMIT: filesystem probe only; no Rust snapshot/save or Swift editor was exercised")


if __name__ == "__main__":
    _probe()
