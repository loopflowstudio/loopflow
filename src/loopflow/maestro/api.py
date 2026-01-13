"""Maestro HTTP API and minimal web UI."""

from __future__ import annotations

import json
import os
import signal
import time
from datetime import datetime
from pathlib import Path

from fastapi import FastAPI, HTTPException
from fastapi.responses import HTMLResponse, JSONResponse, StreamingResponse

from loopflow.maestro.db import DEFAULT_DB_PATH, load_session, load_sessions, load_agents, load_agent
from loopflow.maestro.agents import start_agent, stop_agent, get_agent
from loopflow.worktrees import WorktreeError, list_all

app = FastAPI()


def _format_elapsed(started_at: datetime) -> str:
    delta = datetime.now() - started_at
    seconds = int(delta.total_seconds())

    if seconds < 60:
        return f"{seconds}s"
    if seconds < 3600:
        return f"{seconds // 60}m"
    if seconds < 86400:
        return f"{seconds // 3600}h"
    return f"{seconds // 86400}d"


@app.get("/", response_class=HTMLResponse)
def index() -> str:
    """Minimal web UI for maestro."""
    return """
<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8" />
  <title>Loopflow Maestro</title>
  <style>
    body { font-family: Georgia, serif; background: #f4f1ea; color: #1f1d1a; margin: 0; }
    header { padding: 24px 32px; background: #e3dccf; border-bottom: 1px solid #cdbfae; }
    main { display: grid; grid-template-columns: 1fr 2fr; gap: 24px; padding: 24px 32px; }
    h1 { margin: 0; font-size: 22px; letter-spacing: 0.5px; }
    .panel { background: #fffaf0; border: 1px solid #d9cdbd; border-radius: 8px; padding: 16px; }
    .worktree { padding: 8px 0; border-bottom: 1px solid #e5dbcf; }
    .worktree:last-child { border-bottom: none; }
    .muted { color: #6f665a; }
    .output { white-space: pre-wrap; font-family: "Courier New", monospace; font-size: 13px; }
    button { background: #1f1d1a; color: #fffaf0; border: none; padding: 6px 10px; border-radius: 4px; cursor: pointer; }
  </style>
  <script>
    let selectedSession = null;
    let outputStream = null;

    async function loadWorktrees() {
      const res = await fetch('/api/worktrees');
      const data = await res.json();
      const container = document.getElementById('worktrees');
      container.innerHTML = '';
      data.forEach(wt => {
        const div = document.createElement('div');
        div.className = 'worktree';
        const sessions = wt.sessions || [];
        const sessionLine = sessions.length ? sessions.map(s => `${s.task} (${s.elapsed})`).join(', ') : '—';
        div.innerHTML = `
          <div><strong>${wt.branch}</strong> <span class="muted">${wt.status.ahead}↑ ${wt.status.behind}↓ ${wt.status.dirty ? 'dirty' : 'clean'}</span></div>
          <div class="muted">Sessions: ${sessionLine}</div>
        `;
        if (sessions.length) {
          div.addEventListener('click', () => selectSession(sessions[0].id));
        }
        container.appendChild(div);
      });
    }

    async function selectSession(id) {
      selectedSession = id;
      if (outputStream) {
        outputStream.close();
        outputStream = null;
      }
      const res = await fetch(`/api/sessions/${id}/output`);
      const data = await res.json();
      const out = document.getElementById('output');
      out.textContent = '';
      data.forEach(line => {
        out.textContent += `${line}\\n`;
      });

      outputStream = new EventSource(`/api/sessions/${id}/output?stream=1`);
      outputStream.onmessage = (event) => {
        const payload = JSON.parse(event.data);
        out.textContent += `${payload.line}\\n`;
        out.scrollTop = out.scrollHeight;
      };
    }

    async function refresh() {
      await loadWorktrees();
      if (selectedSession) {
        await selectSession(selectedSession);
      }
    }

    window.onload = () => {
      refresh();
      setInterval(refresh, 5000);
    };
  </script>
</head>
<body>
  <header><h1>Loopflow Maestro</h1></header>
  <main>
    <section class="panel">
      <h2>Worktrees</h2>
      <div id="worktrees"></div>
    </section>
    <section class="panel">
      <h2>Output</h2>
      <div id="output" class="output"></div>
    </section>
  </main>
</body>
</html>
"""


@app.get("/api/sessions")
def sessions() -> JSONResponse:
    items = load_sessions(DEFAULT_DB_PATH, include_completed=True)
    return JSONResponse([s.to_dict() for s in items])


@app.get("/api/sessions/{session_id}")
def session_detail(session_id: str) -> JSONResponse:
    session = load_session(DEFAULT_DB_PATH, session_id)
    if not session:
        raise HTTPException(status_code=404, detail="Session not found")
    return JSONResponse(session.to_dict())


@app.get("/api/sessions/{session_id}/output")
def session_output(session_id: str, stream: bool = False):
    session = load_session(DEFAULT_DB_PATH, session_id)
    if not session:
        raise HTTPException(status_code=404, detail="Session not found")

    log_path = _log_path(session)
    if not log_path.exists():
        return JSONResponse([])

    if not stream:
        return JSONResponse(_read_log_lines(log_path))

    def _iter_events():
        with log_path.open("r", encoding="utf-8") as handle:
            while True:
                line = handle.readline()
                if line:
                    payload = {"line": line.rstrip("\n")}
                    yield f"data: {json.dumps(payload)}\n\n"
                else:
                    time.sleep(0.5)

    return StreamingResponse(_iter_events(), media_type="text/event-stream")


@app.post("/api/sessions/{session_id}/kill")
def kill_session(session_id: str) -> JSONResponse:
    session = load_session(DEFAULT_DB_PATH, session_id)
    if not session or not session.pid:
        raise HTTPException(status_code=404, detail="Session not found or no pid")

    try:
        os.kill(session.pid, signal.SIGTERM)
    except OSError:
        raise HTTPException(status_code=500, detail="Failed to kill process")

    return JSONResponse({"ok": True})


@app.get("/api/worktrees")
def worktrees() -> JSONResponse:
    sessions = load_sessions(DEFAULT_DB_PATH, include_completed=False)
    repos = sorted({s.repo for s in sessions})
    result = []

    for repo in repos:
        try:
            worktrees = list_all(repo)
        except WorktreeError:
            continue

        for wt in worktrees:
            wt_sessions = [
                {
                    "id": s.id,
                    "task": s.task,
                    "status": s.status.value,
                    "elapsed": _format_elapsed(s.started_at),
                }
                for s in sessions
                if s.worktree == wt.path
            ]
            result.append(
                {
                    "branch": wt.branch,
                    "path": str(wt.path),
                    "status": {
                        "ahead": wt.ahead_main,
                        "behind": wt.behind_main,
                        "dirty": wt.is_dirty,
                    },
                    "sessions": wt_sessions,
                }
            )

    return JSONResponse(result)


def _log_path(session) -> Path:
    worktree = session.worktree.name
    return Path.home() / ".lf" / "logs" / worktree / f"{session.id}.log"


def _read_log_lines(log_path: Path) -> list[str]:
    return [line.rstrip("\n") for line in log_path.read_text().splitlines()]


# Agent endpoints


@app.get("/api/agents")
def agents() -> JSONResponse:
    """List all registered agents."""
    items = load_agents(DEFAULT_DB_PATH)
    return JSONResponse([a.to_dict() for a in items])


@app.get("/api/agents/{agent_id}")
def agent_detail(agent_id: str) -> JSONResponse:
    """Get agent details."""
    agent = load_agent(DEFAULT_DB_PATH, agent_id)
    if not agent:
        raise HTTPException(status_code=404, detail="Agent not found")
    return JSONResponse(agent.to_dict())


@app.post("/api/agents/{agent_id}/start")
def agent_start(agent_id: str, repo_root: str | None = None) -> JSONResponse:
    """Start an agent."""
    agent = get_agent(agent_id)
    if not agent:
        raise HTTPException(status_code=404, detail="Agent not found")

    # Use current worktree or provided repo_root
    root = Path(repo_root) if repo_root else Path.cwd()
    result = start_agent(agent_id, root, background=True)

    if not result.success:
        raise HTTPException(status_code=500, detail=result.error or "Failed to start agent")

    return JSONResponse({"ok": True, "log_path": str(result.log_path) if result.log_path else None})


@app.post("/api/agents/{agent_id}/stop")
def agent_stop(agent_id: str) -> JSONResponse:
    """Stop a running agent."""
    success = stop_agent(agent_id)
    if not success:
        raise HTTPException(status_code=500, detail="Failed to stop agent")
    return JSONResponse({"ok": True})
