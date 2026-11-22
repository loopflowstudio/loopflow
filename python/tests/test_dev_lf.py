"""The checkout build entry runs the executable Cargo wrote, with native arguments."""

import json
import os
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]


@pytest.mark.parametrize("isolated", [False, True])
def test_build_target_and_cache_bypass(tmp_path, isolated):
    repo = tmp_path / "source with spaces"
    repo.mkdir()
    (repo / "scripts").mkdir()
    (repo / "scripts/build_cache.py").write_text((ROOT / "scripts/build_cache.py").read_text())
    subprocess.run(["git", "init", "-q", str(repo)], check=True)
    commands = tmp_path / "bin"
    commands.mkdir()
    cargo = commands / "cargo"
    cargo.write_text(
        "#!/usr/bin/env python3\n"
        "import json, os\n"
        "from pathlib import Path\n"
        "root = Path(os.environ['CARGO_TARGET_DIR'])\n"
        "(root / 'debug').mkdir(parents=True)\n"
        "(root / 'build.json').write_text(json.dumps({key: os.environ.get(key) "
        "for key in ['RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'SCCACHE_DISABLE']}))\n"
        "binary = root / 'debug/lf'\n"
        "binary.write_text('#!/usr/bin/env python3\\nimport json, os, sys\\n' "
        "+ 'print(json.dumps([os.environ[\"LF_BIN\"], sys.argv[1:]]))\\n' "
        "+ 'sys.exit(7)\\n')\n"
        "binary.chmod(0o755)\n"
        "print('    Finished test build')\n"
    )
    cargo.chmod(0o755)
    env = {
        **os.environ,
        "PATH": f"{commands}:{os.environ['PATH']}",
        "CARGO_TARGET_DIR": "custom target",
        "RUSTC_WRAPPER": "shared-cache",
        "RUSTC_WORKSPACE_WRAPPER": "workspace-cache",
        "SCCACHE_DISABLE": "0",
        "XDG_CACHE_HOME": str(tmp_path / "cache"),
        "XDG_CONFIG_HOME": str(tmp_path / "config"),
        "HOME": str(tmp_path / "home"),
    }
    env.pop("SCCACHE_CONF", None)
    args = [str(ROOT / "scripts/dev-lf")]
    if isolated:
        args.append("--isolated-build")
    args.extend(["session", "show", "name with spaces"])
    result = subprocess.run(args, cwd=repo, env=env, capture_output=True, text=True)
    target = repo / ("target/isolated" if isolated else "custom target")
    assert result.returncode == 7, result.stderr
    assert json.loads(result.stdout) == [
        str(target / "debug/lf"),
        ["session", "show", "name with spaces"],
    ]
    assert json.loads((target / "build.json").read_text()) == {
        "RUSTC_WRAPPER": "" if isolated else "shared-cache",
        "RUSTC_WORKSPACE_WRAPPER": "" if isolated else "workspace-cache",
        "SCCACHE_DISABLE": "1" if isolated else "0",
    }

    config = tmp_path / "cache/loopflow/sccache.json"
    if isolated:
        assert not config.exists()
    else:
        assert json.loads(config.read_text())["basedirs"] == [str(repo.resolve())]
