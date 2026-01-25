"""Migration registry."""

import sqlite3
from dataclasses import dataclass
from typing import Callable

from loopflow.lfd.migrations import baseline
from loopflow.lfd.migrations import m_2025_01_23_zz_stimulus as stimulus
from loopflow.lfd.migrations import m_2026_01_24_agent_worktree as agent_worktree
from loopflow.lfd.migrations import m_2026_01_24_nullable_goal_area as nullable_goal_area


@dataclass
class Migration:
    version: str
    description: str
    apply: Callable[[sqlite3.Connection], None]


MIGRATIONS = [
    Migration(baseline.SCHEMA_VERSION, baseline.DESCRIPTION, baseline.apply),
    Migration(stimulus.VERSION, stimulus.DESCRIPTION, stimulus.apply),
    Migration(agent_worktree.VERSION, agent_worktree.DESCRIPTION, agent_worktree.apply),
    Migration(
        nullable_goal_area.VERSION,
        nullable_goal_area.DESCRIPTION,
        nullable_goal_area.apply,
    ),
]
