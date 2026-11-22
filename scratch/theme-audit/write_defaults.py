"""Write the chosen proposals into the two default theme files.

Reads the files as they were at the Task's base, so a second run writes the same bytes.
"""
import subprocess
import tempfile
from pathlib import Path

from audit import APP_ROLES, CATALOGUE, ROOT, parse
from proposals import dark_pair, light_repair

BASE = "c4de2038fda5a7fa15cfb975166f56a67d3e0cca"
TERM = ["background", "foreground", "cursor-color", "cursor-text", "selection-background", "selection-foreground"]


def at_base(name):
    rel = (CATALOGUE / name).relative_to(ROOT)
    text = subprocess.run(["git", "-C", str(ROOT), "show", f"{BASE}:{rel}"], check=True,
                          capture_output=True, text=True).stdout
    with tempfile.TemporaryDirectory() as d:
        p = Path(d) / name
        p.write_text(text)
        return parse(p)


def text(t, extra=()):
    lines = [f"# {t['name']}: Loopflow built-in theme."]
    lines += [f"{k} = {t['term'][k]}" for k in TERM]
    lines += [f"palette = {i}={c}" for i, c in enumerate(t["ansi"])]
    lines.append(f"loopflow-appearance = {'dark' if t['dark'] else 'light'}")
    lines += [f"loopflow-{k} = {t['roles'][k]}" for k in APP_ROLES]
    lines += [f"loopflow-{k} = {v}" for k, v in extra]
    lines.append(f"loopflow-selection-opacity = {t['selection-opacity']:g}")
    return "\n".join(lines) + "\n"


def proposed():
    light, dark = at_base("Loopflow Light"), at_base("Loopflow Dark")
    return light_repair(light), dark_pair(dark, light, cool=0.67)


if __name__ == "__main__":
    for t in proposed():
        (CATALOGUE / t["name"]).write_text(text(t))
        print("wrote", t["name"])
