import importlib
import sqlite3
from dataclasses import dataclass
from pathlib import Path
from typing import Callable


@dataclass
class Migration:
    version: str
    description: str
    apply: Callable[[sqlite3.Connection], None]


def load_migrations() -> list[Migration]:
    migrations = []
    for file in sorted(Path(__file__).parent.glob("m_*.py")):
        mod = importlib.import_module(f".{file.stem}", __package__)
        migrations.append(Migration(mod.VERSION, mod.DESCRIPTION, mod.apply))
    return migrations


MIGRATIONS = load_migrations()
