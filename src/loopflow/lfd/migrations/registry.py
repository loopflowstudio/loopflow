"""Migration registry."""

import sqlite3
from dataclasses import dataclass
from typing import Callable

from loopflow.lfd.migrations import baseline
from loopflow.lfd.migrations import m_2025_01_23_zz_stimulus as stimulus
from loopflow.lfd.migrations import m_2026_01_24_agent_worktree as agent_worktree
from loopflow.lfd.migrations import m_2026_01_24_nullable_goal_area as nullable_goal_area
from loopflow.lfd.migrations import m_2026_01_25_agent_paused as agent_paused
from loopflow.lfd.migrations import m_2026_01_26_step_index as step_index
from loopflow.lfd.migrations import m_2026_01_28_wave_stacking as wave_stacking
from loopflow.lfd.migrations import m_2026_01_29_stimuli as stimuli


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
    Migration(agent_paused.VERSION, agent_paused.DESCRIPTION, agent_paused.apply),
    Migration(step_index.SCHEMA_VERSION, step_index.DESCRIPTION, step_index.apply),
    Migration(wave_stacking.VERSION, wave_stacking.DESCRIPTION, wave_stacking.apply),
    Migration(stimuli.VERSION, stimuli.DESCRIPTION, stimuli.apply),
]
