"""Load builtin prompts used by loopflow's own commands."""

from pathlib import Path

_OPS_DIR = Path(__file__).parent / "ops"


def get_builtin_ops_prompt(name: str) -> str:
    """Get a builtin ops prompt by name. Raises FileNotFoundError if not found."""
    return (_OPS_DIR / f"{name}.md").read_text()
