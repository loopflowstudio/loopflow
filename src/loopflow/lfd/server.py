"""Asyncio Unix socket server for lfd daemon."""

import asyncio
import fnmatch
import json
import os
import signal
from asyncio import StreamReader, StreamWriter
from datetime import datetime
from pathlib import Path
from typing import Any

from loopflow.lfd.agents import (
    check_and_run_triggers,
    list_agents,
    start_agent,
    stop_agent,
)
from loopflow.lfd.db import (
    load_agent_runs,
    load_sessions,
    load_sessions_for_repo,
    load_sessions_for_worktree,
    save_session,
    update_dead_runs,
    update_session_status,
)
from loopflow.lfd.models import Session, SessionStatus
from loopflow.lfd.protocol import Event, Request, Response, error, success


class Server:
    def __init__(self, socket_path: Path):
        self.socket_path = socket_path
        self.clients: set[StreamWriter] = set()
        self.subscriptions: dict[StreamWriter, list[str]] = {}
        self._running = False
        self._check_task: asyncio.Task | None = None

    async def start(self) -> None:
        self.socket_path.parent.mkdir(parents=True, exist_ok=True)
        if self.socket_path.exists():
            self.socket_path.unlink()

        server = await asyncio.start_unix_server(
            self._handle_client,
            path=str(self.socket_path),
        )

        self._running = True
        self._check_task = asyncio.create_task(self._periodic_check())

        async with server:
            await server.serve_forever()

    async def stop(self) -> None:
        self._running = False
        if self._check_task:
            self._check_task.cancel()
        for writer in list(self.clients):
            writer.close()
            await writer.wait_closed()
        if self.socket_path.exists():
            self.socket_path.unlink()

    async def _handle_client(self, reader: StreamReader, writer: StreamWriter) -> None:
        self.clients.add(writer)
        try:
            while True:
                line = await reader.readline()
                if not line:
                    break

                try:
                    request = Request.parse(line.decode().strip())
                    response = await self._dispatch(request, writer)
                    writer.write((response.serialize() + "\n").encode())
                    await writer.drain()
                except json.JSONDecodeError:
                    resp = error("Invalid JSON")
                    writer.write((resp.serialize() + "\n").encode())
                    await writer.drain()
                except Exception as e:
                    resp = error(str(e))
                    writer.write((resp.serialize() + "\n").encode())
                    await writer.drain()
        finally:
            self.clients.discard(writer)
            self.subscriptions.pop(writer, None)
            writer.close()

    async def _dispatch(self, request: Request, writer: StreamWriter) -> Response:
        method = request.method
        params = request.params

        if method == "status":
            return await self._handle_status()
        elif method == "agents.list":
            return await self._handle_agents_list()
        elif method == "agents.start":
            return await self._handle_agents_start(params)
        elif method == "agents.stop":
            return await self._handle_agents_stop(params)
        elif method == "sessions.list":
            return await self._handle_sessions_list()
        elif method == "sessions.history":
            return await self._handle_sessions_history(params)
        elif method == "sessions.start":
            return await self._handle_sessions_start(params)
        elif method == "sessions.end":
            return await self._handle_sessions_end(params)
        elif method == "subscribe":
            return await self._handle_subscribe(params, writer)
        elif method == "notify":
            return await self._handle_notify(params)
        elif method == "output.line":
            return await self._handle_output_line(params)
        else:
            return error(f"Unknown method: {method}", request.id)

    async def _handle_status(self) -> Response:
        agents = list_agents()
        sessions = load_sessions(active_only=True)
        runs = load_agent_runs(active_only=True)

        return success({
            "pid": os.getpid(),
            "agents_defined": len(agents),
            "agents_running": len(runs),
            "sessions_active": len(sessions),
        })

    async def _handle_agents_list(self) -> Response:
        agents = list_agents()
        runs = {r.agent_name: r for r in load_agent_runs()}

        result = []
        for agent in agents:
            data = agent.to_dict()
            if agent.name in runs:
                run = runs[agent.name]
                data["status"] = run.status.value
                data["last_run_at"] = run.started_at.isoformat()
                data["iteration"] = run.iteration
                data["pid"] = run.pid
            else:
                data["status"] = "idle"
            result.append(data)

        return success(result)

    async def _handle_agents_start(self, params: dict) -> Response:
        name = params.get("name")
        if not name:
            return error("Missing 'name' parameter")

        result = await start_agent(name)
        if result.error:
            return error(result.error)

        await self._broadcast(Event("agent.started", {"name": name, "pid": result.pid}))
        return success({"name": name, "pid": result.pid})

    async def _handle_agents_stop(self, params: dict) -> Response:
        name = params.get("name")
        if not name:
            return error("Missing 'name' parameter")

        if stop_agent(name):
            await self._broadcast(Event("agent.stopped", {"name": name}))
            return success({"name": name})
        else:
            return error(f"Agent '{name}' not running")

    async def _handle_sessions_list(self) -> Response:
        sessions = load_sessions()
        return success([s.to_dict() for s in sessions])

    async def _handle_sessions_history(self, params: dict) -> Response:
        """Return session history for a worktree or repo."""
        worktree = params.get("worktree")
        repo = params.get("repo")
        limit = params.get("limit", 20)

        if worktree:
            sessions = load_sessions_for_worktree(worktree, limit)
        elif repo:
            sessions = load_sessions_for_repo(repo, limit)
        else:
            sessions = load_sessions()[:limit]

        return success([s.to_dict() for s in sessions])

    async def _handle_sessions_start(self, params: dict) -> Response:
        """Record a session start."""
        session_data = params.get("session")
        if not session_data:
            return error("Missing 'session' parameter")

        session = Session.from_dict(session_data)
        save_session(session)
        await self._broadcast(Event("session.started", {"id": session.id, "task": session.task}))
        return success({"id": session.id})

    async def _handle_sessions_end(self, params: dict) -> Response:
        """Record a session end."""
        session_id = params.get("session_id")
        status_str = params.get("status")

        if not session_id or not status_str:
            return error("Missing 'session_id' or 'status' parameter")

        status = SessionStatus(status_str)
        update_session_status(session_id, status)
        await self._broadcast(Event("session.ended", {"id": session_id, "status": status_str}))
        return success({"id": session_id})

    async def _handle_subscribe(self, params: dict, writer: StreamWriter) -> Response:
        events = params.get("events", [])
        self.subscriptions[writer] = events
        return success({"subscribed": events})

    async def _handle_notify(self, params: dict) -> Response:
        """Accept external events and broadcast to subscribers."""
        event_name = params.get("event")
        event_data = params.get("data", {})

        if not event_name:
            return error("Missing 'event' parameter")

        await self._broadcast(Event(event_name, event_data))
        return success({"event": event_name})

    async def _handle_output_line(self, params: dict) -> Response:
        """Accept output lines from collector and broadcast to subscribers."""
        session_id = params.get("session_id")
        text = params.get("text")

        if not session_id or text is None:
            return error("Missing 'session_id' or 'text' parameter")

        await self._broadcast(Event("output.line", {
            "session_id": session_id,
            "text": text,
            "timestamp": datetime.now().isoformat(),
        }))
        return success({})

    async def _broadcast(self, event: Event) -> None:
        message = (event.serialize() + "\n").encode()
        for writer, patterns in list(self.subscriptions.items()):
            if any(fnmatch.fnmatch(event.event, p) for p in patterns):
                try:
                    writer.write(message)
                    await writer.drain()
                except Exception:
                    self.clients.discard(writer)
                    self.subscriptions.pop(writer, None)

    async def _periodic_check(self) -> None:
        """Periodically check agent triggers and update dead processes."""
        while self._running:
            try:
                await asyncio.sleep(30)
                update_dead_runs()
                await check_and_run_triggers()
            except asyncio.CancelledError:
                break
            except Exception:
                pass


async def run_server(socket_path: Path) -> None:
    """Main daemon entry point. Runs until terminated."""
    server = Server(socket_path)

    loop = asyncio.get_event_loop()
    for sig in (signal.SIGTERM, signal.SIGINT):
        loop.add_signal_handler(sig, lambda: asyncio.create_task(server.stop()))

    await server.start()
