"""Load builtin prompts used by loopflow's own commands."""

from importlib.resources import files


def get_builtin_prompt(name: str) -> str:
    """Get a builtin prompt by name. Raises FileNotFoundError if not found."""
    return files(__package__).joinpath(f"{name}.txt").read_text()
