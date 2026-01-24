"""Migration registry."""

import sqlite3
from dataclasses import dataclass
from typing import Callable

from loopflow.lfd.migrations import (
    baseline,
    m_2025_01_23_zz_agent_worktree,
    m_2026_01_23_activation_queue,
)


@dataclass
class Migration:
    version: str
    description: str
    apply: Callable[[sqlite3.Connection], None]


MIGRATIONS = [
    Migration(baseline.SCHEMA_VERSION, baseline.DESCRIPTION, baseline.apply),
    Migration(
        m_2025_01_23_zz_agent_worktree.VERSION,
        m_2025_01_23_zz_agent_worktree.DESCRIPTION,
        m_2025_01_23_zz_agent_worktree.apply,
    ),
    Migration(
        m_2026_01_23_activation_queue.VERSION,
        m_2026_01_23_activation_queue.DESCRIPTION,
        m_2026_01_23_activation_queue.apply,
    ),
]
