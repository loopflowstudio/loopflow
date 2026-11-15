"""Compare registry resolution and assembled-source packaging without uploading."""

import os
from pathlib import Path
import subprocess
import tempfile


def main() -> None:
    source = Path(__file__).resolve().parents[1] / "rust/loopflow/src/command.rs"
    with tempfile.TemporaryDirectory(prefix="lf650-publish-", dir="/tmp") as temporary:
        root = Path(temporary)
        for name in ("leaf", "app", "assembled"):
            (root / name / "src").mkdir(parents=True)
        (root / "leaf/src/lib.rs").write_text(source.read_text())
        (root / "app/src/lib.rs").write_text("pub use lf650_leaf::*;\n")
        (root / "assembled/src/lib.rs").write_text(source.read_text())
        for name, package in (("leaf", "lf650-leaf"), ("app", "lf650-app"), ("assembled", "lf650-app")):
            manifest = (f'[package]\nname="{package}"\nversion="0.1.0"\nedition="2021"\n'
                        'description="LOO-650 disposable packaging probe"\nlicense="MIT"\n')
            if name == "app":
                manifest += '[dependencies]\nlf650-leaf={path="../leaf",version="=0.1.0"}\n'
            (root / name / "Cargo.toml").write_text(manifest)
        env = {key: value for key, value in os.environ.items()
               if not key.startswith(("LF_", "CARGO_", "SCCACHE_"))}
        env.update(RUSTC_WRAPPER="", CARGO_INCREMENTAL="0")
        for name in ("leaf", "app", "assembled"):
            result = subprocess.run(
                ["nice", "-n", "10", "cargo", "package", "--offline", "--jobs", "1"],
                cwd=root / name, env=env, capture_output=True, text=True,
            )
            print(f"{name}: exit {result.returncode}\n{result.stdout}{result.stderr}")
            if (name == "app") == (result.returncode == 0):
                raise RuntimeError(f"unexpected packaging result for {name}")


if __name__ == "__main__":
    main()
