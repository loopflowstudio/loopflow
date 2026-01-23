"""Migration registry."""

import sqlite3
from dataclasses import dataclass
from typing import Callable

from loopflow.lfd.migrations import baseline


@dataclass
class Migration:
    version: str
    description: str
    apply: Callable[[sqlite3.Connection], None]


MIGRATIONS = [
    Migration(baseline.SCHEMA_VERSION, baseline.DESCRIPTION, baseline.apply),
]
