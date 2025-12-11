"""Builtin prompts for loopflow commands.

These are prompts used by loopflow's own commands (lf pr create, etc.),
not user-defined tasks in .lf/.
"""

from pathlib import Path

_BUILTINS_DIR = Path(__file__).parent


def get_builtin_prompt(name: str) -> str:
    """Get a builtin prompt by name. Raises FileNotFoundError if not found."""
    prompt_file = _BUILTINS_DIR / f"{name}.txt"
    return prompt_file.read_text()
