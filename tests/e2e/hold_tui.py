"""Hold a command in a PTY for a walkthrough driven from another shell.

    uv run python tests/e2e/hold_tui.py <dir> <cwd> lf resume --task <task>

Append lines to <dir>/in: `enter <text>`, `type <text>`, `hex <bytes>` (1b is
Esc, 1b5b42 Down, 0d Enter) or `quit`. <dir>/screen.txt is the rendered
screen, rewritten a few times a second.
"""
import os, sys, time
from pathlib import Path
from tui import _Tui

d = Path(sys.argv[1]); cwd = Path(sys.argv[2]); cmd = sys.argv[3:]
d.mkdir(parents=True, exist_ok=True)
inp = d / "in"; inp.write_text(""); pos = 0
tui = _Tui(cmd, cwd, dict(os.environ))
try:
    while tui.child.poll() is None:
        tui.drain(0.3)
        (d / "screen.txt").write_text("\n".join(l.rstrip() for l in tui.screen.display))
        data = inp.read_text()
        for line in data[pos:].splitlines():
            if line.startswith("enter "): tui.enter(line[6:])
            elif line.startswith("type "): os.write(tui.master, line[5:].encode())
            elif line.startswith("hex "): os.write(tui.master, bytes.fromhex(line[4:]))
            elif line == "quit": raise SystemExit
        pos = len(data)
finally:
    tui.drain(0.5)
    (d / "screen.txt").write_text("\n".join(l.rstrip() for l in tui.screen.display))
    (d / "raw").write_bytes(tui.raw)
    (d / "exit").write_text(str(tui.child.poll()))
    tui.close()
