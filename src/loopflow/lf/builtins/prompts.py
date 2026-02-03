"""Load builtin prompts used by loopflow's own commands."""

from pathlib import Path

_OPS_DIR = Path(__file__).parent / "ops"

# Load all prompts at import time. This prevents FileNotFoundError when
# the worktree is moved mid-execution (e.g., during `lf ops next`).
_OPS_PROMPTS = {path.stem: path.read_text() for path in _OPS_DIR.glob("*.md")}


def get_builtin_ops_prompt(name: str) -> str:
    if name not in _OPS_PROMPTS:
        raise FileNotFoundError(f"Builtin ops prompt not found: {name}")
    return _OPS_PROMPTS[name]
