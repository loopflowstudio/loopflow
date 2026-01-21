from dataclasses import dataclass
import importlib
from pathlib import Path
import sqlite3
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
