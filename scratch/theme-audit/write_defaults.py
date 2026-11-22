"""Write the chosen proposals into the two default theme files.

Reads the two defaults as they were at the Task's base, so a second run writes the same bytes.
"""
import re
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from audit import CATALOGUE, FILE_ROLES, ROOT, load, parse, titlebar_rule  # noqa: E402
from proposals import dark_pair, light_repair  # noqa: E402

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


def text(t):
    lines = [f"# {t['name']}: Loopflow built-in theme."]
    lines += [f"{k} = {t['term'][k]}" for k in TERM]
    lines += [f"palette = {i}={c}" for i, c in enumerate(t["ansi"])]
    lines.append(f"loopflow-appearance = {'dark' if t['dark'] else 'light'}")
    lines += [f"loopflow-{k} = {t['roles'][k]}" for k in FILE_ROLES if k in t["roles"]]
    lines.append(f"loopflow-selection-opacity = {t['selection-opacity']:g}")
    return "\n".join(lines) + "\n"


def proposed():
    light, dark = at_base("Loopflow Light"), at_base("Loopflow Dark")
    return light_repair(light), dark_pair(dark, light, cool=0.67)


def state_titlebars():
    """Every other built-in states the bar the rule gives it, after its accent ink."""
    for t in load():
        r, path = t["roles"], CATALOGUE / t["name"]
        if "titlebar" in r:
            continue
        bar = titlebar_rule(r["accent"], r["background"], t["dark"])
        source, count = re.subn(r"(?m)^(loopflow-accent-ink\s*=.*\n)", rf"\1loopflow-titlebar = {bar}\n", path.read_text())
        if count != 1:
            raise SystemExit(f"{t['name']}: no single accent-ink line")
        path.write_text(source)
        print(t["name"], bar)


if __name__ == "__main__":
    for t in proposed():
        (CATALOGUE / t["name"]).write_text(text(t))
        print("wrote", t["name"])
    state_titlebars()
