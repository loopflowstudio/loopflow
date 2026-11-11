"""Build scratch/process-audit.html from live ps, lsof and lf's own records.

Read-only: it signals nothing. Run with `python3 -I scratch/process_audit.py`.
"""

import html
import json
import os
import re
import subprocess
import sys
import time
from collections import Counter, defaultdict
from pathlib import Path

OUT = Path(__file__).resolve().parent / "process-audit.html"
SID = re.compile(r"session_[0-9a-f]{32}|\b[0-9a-f]{32}\b")


def run(args):
    return subprocess.run(args, capture_output=True, text=True, stdin=subprocess.DEVNULL).stdout


def seconds(etime):
    days, _, rest = etime.rpartition("-")
    parts = [int(p) for p in rest.split(":")]
    while len(parts) < 3:
        parts.insert(0, 0)
    return (int(days) if days else 0) * 86400 + parts[0] * 3600 + parts[1] * 60 + parts[2]


def age(s):
    if s >= 86400:
        return f"{s // 86400}d {s % 86400 // 3600}h"
    if s >= 3600:
        return f"{s // 3600}h {s % 3600 // 60}m"
    return f"{s // 60}m"


# ---- Readings -------------------------------------------------------------

def read_ps():
    procs = {}
    for line in run(["ps", "-axww", "-o", "pid=,ppid=,pgid=,etime=,command="]).splitlines():
        pid, ppid, pgid, etime, command = line.split(None, 4)
        procs[int(pid)] = {
            "pid": int(pid),
            "ppid": int(ppid),
            "pgid": int(pgid),
            "age": seconds(etime),
            "command": command,
        }
    return procs


def lf_binary(command):
    """The lf executable path when this command line runs lf, else None."""
    # The first `/lf` or `/lf-<suffix>` word ends the path; app paths hold spaces.
    m = re.match(r"^(lf|/.*?/lf(?:-[A-Za-z0-9]+)?)(?=\s|$)", command)
    return m.group(1) if m else None


def read_lsof():
    """fd tables: pid -> list of (fd, type, addr, name)."""
    tables = defaultdict(list)
    pid = None
    fd = typ = addr = None
    for line in run(["lsof", "-n", "-P", "-F", "pftdn"]).splitlines():
        tag, value = line[:1], line[1:]
        if tag == "p":
            pid = int(value)
        elif tag == "f":
            fd, typ, addr = value, None, None
        elif tag == "t":
            typ = value
        elif tag == "d":
            addr = value
        elif tag == "n" and pid is not None:
            tables[pid].append((fd, typ, addr, value))
    return tables


def lf_json(args):
    out = run(["lf", *args, "--json"])
    try:
        return json.loads(out)
    except json.JSONDecodeError:
        return None


# ---- Classification -------------------------------------------------------

def binary_kind(path):
    if path in ("lf", "/Applications/Loopflow.app/Contents/MacOS/lf") or path == str(Path.home() / ".local/bin/lf"):
        return "installed"
    if "Loopflow Dev.app" in path:
        return "Dev app"
    if "/demos/" in path or "Redemo" in path or "/tmp/loo-" in path:
        return "demo"
    if re.search(r"^/(private/)?tmp/|/T/\.tmp|/var/folders/", path):
        return "test fixture"
    if "/target/" in path:
        return "checkout build"
    return "other"


def role(args):
    if args.startswith("session serve"):
        return "session serve"
    if args.startswith("session attach"):
        return "session attach"
    if args.startswith("session observe-status"):
        return "session observe-status"
    if args.startswith("__codex-observe"):
        return "__codex-observe"
    if args.startswith("__context-block"):
        return "__context-block"
    if args.startswith("server run"):
        return "server run"
    if args.startswith("task run") or args.startswith("--batch") or " -b " in f" {args} ":
        return "run (task or headless step)"
    return "viewer or command"


LOSES = {
    "session serve": "The running agent and any turn in flight. `lf resume <ID>` continues from history.",
    "session attach": "Nothing; the conversation keeps running detached.",
    "session observe-status": "Nothing when its client is gone or it was replaced; otherwise the pane's working/waiting status until shown again.",
    "__codex-observe": "Turn and usage records for that Codex conversation; the conversation keeps going.",
    "__context-block": "Task updates delivered into that Claude conversation until it next starts or resumes.",
    "server run": "Desktop's connection until the server starts again.",
    "run (task or headless step)": "The Graph step in progress; `lf task status` shows what it reached.",
    "viewer or command": "That terminal's view or command; a detached owner keeps the conversation.",
}


def main():
    now = time.strftime("%Y-%m-%d %H:%M %Z")
    procs = read_ps()
    lsof = read_lsof()
    sessions = lf_json(["session", "list", "--all", "--interactive", "all", "--limit", "0"]) or []
    tree = (lf_json(["process", "list"]) or {}).get("nodes", [])
    # Read-only: what `lf process rollover` would move and why each stays.
    rollover = lf_json(["process", "rollover", "--dry-run"]) or {}
    held = {
        entry["process"]["pid"]: entry["reason"]
        for key in ("stays", "next_step", "moved")
        for entry in rollover.get(key, [])
    }

    by_session = {s["id"]: s for s in sessions}
    by_short = {s["id"].removeprefix("session_"): s for s in sessions}
    session_by_agent = {s["agent_process_id"]: s for s in sessions if s.get("agent_process_id")}
    node_by_pid = {n["pid"]: n for n in tree if n.get("pid")}
    children_of_node = defaultdict(list)
    for n in tree:
        children_of_node[n.get("parent_id")].append(n)
    children = defaultdict(list)
    for p in procs.values():
        children[p["ppid"]].append(p)

    # Pipe and socket ends: address -> pids that hold it.
    holders = defaultdict(set)
    connected_to = defaultdict(set)
    for pid, rows in lsof.items():
        for _, typ, addr, name in rows:
            if typ in ("PIPE", "unix") and addr:
                holders[addr].add(pid)
            if typ == "unix" and name.startswith("->"):
                connected_to[name[2:]].add(pid)

    def app_alive(pid):
        cmd = procs.get(pid, {}).get("command", "")
        return re.search(r"\.app/Contents/MacOS/Loopflow", cmd) is not None and "/lf " not in cmd

    rows = []
    for p in procs.values():
        path = lf_binary(p["command"])
        if not path:
            continue
        args = p["command"][len(path):].strip()
        r = role(args)
        found = SID.search(args)
        session = None
        if found:
            key = found.group(0)
            session = by_session.get(key) or by_short.get(key.removeprefix("session_"))
        node = node_by_pid.get(p["pid"])
        # An owner's agent sits below the lf it runs: owner -> lf -> agent_process.
        frontier = [node] if node is not None else []
        while session is None and frontier:
            below = [c for n in frontier for c in children_of_node.get(n["id"], [])]
            session = next((session_by_agent[c["id"]] for c in below if c["id"] in session_by_agent), None)
            frontier = below
        superseded = any(
            "superseded" in name for fd, typ, _, name in lsof.get(p["pid"], []) if fd == "txt"
        )
        note = ""
        healthy = True
        if r == "session serve":
            provider = [c for c in children[p["pid"]] if not lf_binary(c["command"])]
            # A viewer holds a unix socket connected to one of the owner's.
            ends = {addr for _, typ, addr, _ in lsof.get(p["pid"], []) if typ == "unix" and addr}
            viewers = {pid for addr in ends for pid in connected_to.get(addr, set()) if pid != p["pid"]}
            note = (
                f"provider {'alive: ' + provider[0]['command'][:60] if provider else 'none'}; "
                f"{len(viewers)} viewer{'s' if len(viewers) != 1 else ''} attached"
            )
            reason = held.get(p["pid"])
            if reason:
                note += f"; rollover: {reason}"
            # Input or a Graph holding the conversation is work in progress, not a leftover.
            healthy = bool(viewers) or p["age"] < 86400 or "holds" in (reason or "")
        elif r == "session observe-status":
            stdin = [(addr, name) for fd, typ, addr, name in lsof.get(p["pid"], []) if fd == "0" and typ == "PIPE"]
            writers = set()
            for _, name in stdin:
                writers |= holders.get(name[2:], set()) if name.startswith("->") else set()
            apps = [w for w in writers if app_alive(w)]
            others = len(writers) - len(apps)
            note = (
                f"client {'alive' if apps else 'gone'}; stdin write end held by "
                f"{others} other process{'es' if others != 1 else ''}"
            )
            healthy = bool(apps)
        elif r in ("__codex-observe", "__context-block"):
            parent = procs.get(p["ppid"])
            pcmd = parent["command"].split()[0] if parent else "launchd"
            note = f"parent {pcmd.rsplit('/', 1)[-1]} ({p['ppid']})"
            healthy = p["ppid"] != 1
        elif r == "session attach":
            healthy = p["ppid"] != 1
            note = "terminal gone" if p["ppid"] == 1 else ""
        rows.append({
            **p,
            "path": path,
            "kind": binary_kind(path),
            "role": r,
            "session": session,
            "note": note,
            "healthy": healthy,
            "superseded": superseded,
        })

    # Observers sharing one Session and terminal: all but the newest were replaced.
    observers = defaultdict(list)
    for row in rows:
        if row["role"] == "session observe-status":
            m = re.search(r"--terminal (\S+)", row["command"])
            observers[(SID.search(row["command"]).group(0), m.group(1) if m else "")].append(row)
    for group in observers.values():
        group.sort(key=lambda r: r["age"])
        for older in group[1:]:
            older["note"] += "; replaced by a newer observer"
            older["healthy"] = False

    write_report(now, rows)
    print(f"{len(rows)} lf processes -> {OUT}")
    for (r, kind), count in sorted(Counter((x["role"], x["kind"]) for x in rows).items(), key=lambda kv: -kv[1]):
        print(f"{count:4} {r} [{kind}]")


# ---- Report -------------------------------------------------------------------

def advice(rows):
    e = html.escape

    def pids(match):
        found = sorted((x for x in rows if match(x)), key=lambda x: -x["age"])
        return found, " ".join(str(x["pid"]) for x in found)

    fixtures, fixture_pids = pids(lambda x: x["kind"] == "test fixture")
    observers, observer_pids = pids(lambda x: x["role"] == "session observe-status" and not x["healthy"])
    drafts, _ = pids(lambda x: x["role"] == "session serve" and "native input holds" in x["note"])
    graphs, _ = pids(lambda x: x["role"] == "session serve" and "Graph holds" in x["note"])
    empty, empty_pids = pids(lambda x: x["role"] == "session serve" and x["kind"] == "installed"
                             and "keeps its lf until it exits" in x["note"])
    demos, demo_pids = pids(lambda x: x["kind"] in ("demo", "Dev app"))
    out = ["<h2>What to do</h2><table><tr><th>Group</th><th class=num>Count</th><th>Why it is still running</th><th>Recommendation</th></tr>"]

    def row(name, group, why, what):
        out.append(f"<tr><td>{name}</td><td class=num>{len(group)}</td><td>{why}</td><td>{what}</td></tr>")

    row(
        "Test fixture processes (<code>/tmp/lf-7501-*</code>)", fixtures,
        "<code>transport_tests</code>' <code>Fixture</code> started detached owners and never ended them; their "
        "temporary homes were deleted under them. Fixed: the fixture now ends what its store recorded when it drops.",
        f"Safe to stop; they serve nothing a person can open. <code>kill {e(fixture_pids)}</code>" if fixtures else "Nothing left.",
    )
    row(
        "<code>session observe-status</code> with no client or replaced", observers,
        "Desktop's stdin pipe for each observer was inheritable, so every pane forked afterwards (and each "
        "detached owner it started) kept the write end open; the observer never saw end of input. "
        "Fixed in both places: Desktop pipes are close-on-exec and a detached owner drops descriptors it inherited.",
        f"Safe to stop; they record nothing. <code>kill {e(observer_pids)}</code>" if observers else "Nothing left.",
    )
    row(
        "Owners held by typed input", drafts,
        "Bytes typed into the provider that it never acknowledged as one exact prompt: a draft, an edit, "
        "arrow keys or Esc. Idle ending and <code>lf process rollover</code> both keep these, because a dead "
        "editor can't be rebuilt from history. Most also run an <code>lf</code> replaced on disk since they started.",
        "Open each with <code>lf resume &lt;ID&gt;</code>, then send or clear what is there; it ends on its own "
        "after <code>session.end_detached_after</code>. Stopping one loses that input.",
    )
    row(
        "Owners held by a running Graph", graphs,
        "A Graph step runs in the conversation.",
        "Leave it; <code>lf task status</code> shows the step.",
    )
    row(
        "Owners with no open Session", empty,
        "Rollover reads no unfinished Session on these owners (Desktop shells, or a conversation that ended "
        "while its owner kept serving).",
        f"Read each with <code>lf process list</code> before deciding. PIDs: <code>{e(empty_pids)}</code>" if empty else "Nothing left.",
    )
    row(
        "Demo and Dev app processes", demos,
        "A demo keeps its processes while a person's input there is unsettled.",
        f"Read the demo's note under its Task's <code>scratch/</code> first. PIDs: <code>{e(demo_pids)}</code>" if demos else "Nothing left.",
    )
    out.append("</table>")
    return out


def write_report(now, rows):
    e = html.escape
    roles = Counter(r["role"] for r in rows)
    kinds = Counter(r["kind"] for r in rows)
    out = [
        "<!doctype html><meta charset=utf-8><title>lf process audit</title>",
        "<style>body{font:14px/1.45 -apple-system,system-ui,sans-serif;margin:2rem auto;max-width:1200px;color:#1d1d1f;padding:0 1rem}"
        "table{border-collapse:collapse;width:100%;margin:.5rem 0 2rem}td,th{border-bottom:1px solid #e5e5ea;padding:.3rem .5rem;text-align:left;vertical-align:top}"
        "th{background:#f5f5f7}code{font:12px ui-monospace,Menlo,monospace}.stale{color:#b3261e}.num{text-align:right}"
        "details{margin:.4rem 0}summary{cursor:pointer;font-weight:600}h2{margin-top:2.5rem}</style>",
        f"<h1>lf processes on this Machine</h1><p>Read {e(now)} from <code>ps</code>, <code>lsof</code>, "
        "<code>lf session list --all --interactive all --limit 0 --json</code>, <code>lf process list --json</code> and <code>lf process rollover --dry-run --json</code>. "
        f"<b>{len(rows)}</b> processes run an <code>lf</code> binary (hooks and commands start and end during the few seconds "
        "of the read, so a recount differs by those); recount with "
        "<code>ps -axww -o command= | grep -cE '^(lf|/.*/lf(-[A-Za-z0-9]+)?)( |$)'</code>. "
        "Nothing here was stopped; stop one by PID after reading what it serves.</p>",
        *advice(rows),
        "<h2>By role</h2><table><tr><th>Role</th><th class=num>Count</th><th class=num>Looks stale</th><th>Stopping one loses</th></tr>",
    ]
    for r, count in roles.most_common():
        stale = sum(1 for x in rows if x["role"] == r and not x["healthy"])
        out.append(f"<tr><td><code>{e(r)}</code></td><td class=num>{count}</td><td class='num{' stale' if stale else ''}'>{stale}</td><td>{e(LOSES[r])}</td></tr>")
    out.append("</table><h2>By binary</h2><table><tr><th>Started from</th><th class=num>Count</th><th>Paths</th></tr>")
    for kind, count in kinds.most_common():
        paths = Counter(x["path"] for x in rows if x["kind"] == kind)
        listed = "<br>".join(f"<code>{e(p)}</code> × {n}" for p, n in paths.most_common(8))
        out.append(f"<tr><td>{e(kind)}</td><td class=num>{count}</td><td>{listed}</td></tr>")
    out.append("</table>")
    out.append(
        "<p><b>Looks stale</b> means: an owner more than a day old with no viewer attached and no input or Graph holding it; an observer whose "
        "Desktop client is gone or that a newer observer replaced; a hook or observer whose parent exited; "
        "an attach whose terminal is gone. An old owner with a live provider can still hold a turn, draft or "
        "pending request, so read it with <code>lf resume &lt;ID&gt;</code> before deciding.</p>"
    )
    for r, _ in roles.most_common():
        group = sorted((x for x in rows if x["role"] == r), key=lambda x: (x["healthy"], -x["age"]))
        stale = sum(1 for x in group if not x["healthy"])
        out.append(f"<h2><code>{e(r)}</code> — {len(group)}, {stale} looking stale</h2><p>Stopping one loses: {e(LOSES[r])}</p>")
        out.append("<table><tr><th>PID</th><th>Age</th><th>Session</th><th>State</th><th>Reading</th><th>Binary</th></tr>")
        for x in group:
            s = x["session"]
            title = f"{e(s.get('title') or '')}<br><code>{e(s['id'])}</code>" if s else "—"
            state = e(s["state"]) if s else "—"
            note = e(x["note"]) + (" ; binary replaced on disk since start" if x["superseded"] else "")
            cls = " class=stale" if not x["healthy"] else ""
            out.append(
                f"<tr{cls}><td>{x['pid']}</td><td>{age(x['age'])}</td><td>{title}</td><td>{state}</td>"
                f"<td>{note}</td><td>{e(x['kind'])}<br><code title='{e(x['command'])}'>{e(x['path'])}</code></td></tr>"
            )
        out.append("</table>")
    OUT.write_text("\n".join(out))


if __name__ == "__main__":
    sys.exit(main())
