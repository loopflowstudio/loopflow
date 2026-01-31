"""Asyncio Unix socket server for lfd daemon."""

import asyncio
import fnmatch
import json
import signal
from asyncio import StreamReader, StreamWriter
from datetime import datetime
from pathlib import Path

from loopflow.lfd.daemon import metrics
from loopflow.lfd.daemon.manager import Manager, load_manager_config
from loopflow.lfd.daemon.protocol import Event, Request, Response, error, success
from loopflow.lfd.daemon.status import compute_status
from loopflow.lfd.db import update_dead_processes
from loopflow.lfd.flow_run import cleanup_stale_runs
from loopflow.lfd.git_hooks import hooks_status, install_hooks
from loopflow.lfd.models import StepRun, StepRunStatus
from loopflow.lfd.pr_poller import PRPoller
from loopflow.lfd.step_run import (
    load_step_runs,
    load_step_runs_for_repo,
    load_step_runs_for_worktree,
    save_step_run,
    update_step_run_status,
)
from loopflow.lfd.wave import run_cron_check, run_watch_check
from loopflow.lfd.worktree_state import get_worktree_state_service


class Server:
    def __init__(self, socket_path: Path):
        self.socket_path = socket_path
        self.clients: set[StreamWriter] = set()
        self.subscriptions: dict[StreamWriter, list[str]] = {}
        self._running = False
        self._check_task: asyncio.Task | None = None
        self._poller_task: asyncio.Task | None = None
        self.manager = Manager(load_manager_config())
        self.pr_poller = PRPoller()

    async def start(self) -> None:
        self.socket_path.parent.mkdir(parents=True, exist_ok=True)
        if self.socket_path.exists():
            self.socket_path.unlink()

        # Cleanup stale state from previous runs
        update_dead_processes()
        cleanup_stale_runs()

        server = await asyncio.start_unix_server(
            self._handle_client,
            path=str(self.socket_path),
        )

        self._running = True
        self._check_task = asyncio.create_task(self._periodic_check())
        self._poller_task = asyncio.create_task(self._run_pr_poller())

        async with server:
            await server.serve_forever()

    async def stop(self) -> None:
        self._running = False
        if self._check_task:
            self._check_task.cancel()
        if self._poller_task:
            self._poller_task.cancel()
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
                    try:
                        resp = error("Invalid JSON")
                        writer.write((resp.serialize() + "\n").encode())
                        await writer.drain()
                    except (ConnectionResetError, BrokenPipeError, OSError):
                        break
                except (ConnectionResetError, BrokenPipeError, OSError):
                    break  # Client disconnected, exit cleanly
                except Exception as e:
                    try:
                        resp = error(str(e))
                        writer.write((resp.serialize() + "\n").encode())
                        await writer.drain()
                    except (ConnectionResetError, BrokenPipeError, OSError):
                        break
        except (ConnectionResetError, BrokenPipeError, OSError):
            pass  # Client disconnected, ignore
        finally:
            self.clients.discard(writer)
            self.subscriptions.pop(writer, None)
            try:
                writer.close()
                await writer.wait_closed()
            except Exception:
                pass  # Ignore any close errors

    async def _dispatch(self, request: Request, writer: StreamWriter) -> Response:
        metrics.increment("socket_requests")
        method = request.method
        params = request.params

        if method == "status":
            return await self._handle_status()
        elif method == "step_runs.list":
            return await self._handle_step_runs_list()
        elif method == "step_runs.history":
            return await self._handle_step_runs_history(params)
        elif method == "step_runs.start":
            return await self._handle_step_runs_start(params)
        elif method == "step_runs.end":
            return await self._handle_step_runs_end(params)
        elif method == "subscribe":
            return await self._handle_subscribe(params, writer)
        elif method == "notify":
            return await self._handle_notify(params)
        elif method == "output.line":
            return await self._handle_output_line(params)
        elif method == "scheduler.status":
            return await self._handle_manager_status()
        elif method == "scheduler.acquire":
            return await self._handle_manager_acquire(params)
        elif method == "scheduler.release":
            return await self._handle_manager_release(params)
        elif method == "worktrees.list":
            return await self._handle_worktrees_list(params)
        elif method == "worktrees.changed":
            return await self._handle_worktrees_changed(params)
        else:
            return error(f"Unknown method: {method}", request.id)

    async def _handle_status(self) -> Response:
        return success(compute_status())

    async def _handle_step_runs_list(self) -> Response:
        step_runs = load_step_runs()
        return success([s.to_dict() for s in step_runs])

    async def _handle_step_runs_history(self, params: dict) -> Response:
        """Return step run history for a worktree or repo."""
        worktree = params.get("worktree")
        repo = params.get("repo")
        limit = params.get("limit", 20)

        if worktree:
            step_runs = load_step_runs_for_worktree(worktree, limit)
        elif repo:
            step_runs = load_step_runs_for_repo(repo, limit)
        else:
            step_runs = load_step_runs()[:limit]

        return success([s.to_dict() for s in step_runs])

    async def _handle_step_runs_start(self, params: dict) -> Response:
        """Record a step run start."""
        step_run_data = params.get("step_run")
        if not step_run_data:
            return error("Missing 'step_run' parameter")

        step_run = StepRun.from_dict(step_run_data)
        save_step_run(step_run)
        await self._broadcast(
            Event(
                "session.started",
                {
                    "id": step_run.id,
                    "step": step_run.step,
                    "worktree": step_run.worktree,
                },
            )
        )
        return success({"id": step_run.id})

    async def _handle_step_runs_end(self, params: dict) -> Response:
        """Record a step run end."""
        step_run_id = params.get("step_run_id")
        status_str = params.get("status")

        if not step_run_id or not status_str:
            return error("Missing 'step_run_id' or 'status' parameter")

        status = StepRunStatus(status_str)
        update_step_run_status(step_run_id, status)
        await self._broadcast(Event("session.ended", {"id": step_run_id, "status": status_str}))
        return success({"id": step_run_id})

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

        # Special handling for git events from hooks
        if event_name.startswith("git."):
            return await self._handle_git_event(event_name, event_data)

        await self._broadcast(Event(event_name, event_data))
        return success({"event": event_name})

    async def _handle_git_event(self, event_name: str, data: dict) -> Response:
        """Handle git hook notifications with rich event emission."""
        repo = data.get("repo")
        branch = data.get("branch")

        if not repo:
            return error("Missing 'repo' in git event data")

        repo_path = Path(repo)
        service = get_worktree_state_service()
        service.invalidate(repo_path)

        # Emit rich event if we have a branch
        if branch:
            worktree_status = service.get_one(repo_path, branch)
            reason = event_name.replace("git.", "")  # "commit", "checkout", etc.
            await self._broadcast(
                Event(
                    "worktree.updated",
                    {
                        "branch": branch,
                        "reason": reason,
                        "repo": str(repo_path),
                        "worktree": worktree_status,
                    },
                )
            )

        return success({"event": event_name, "branch": branch})

    async def _handle_output_line(self, params: dict) -> Response:
        """Accept output lines from collector and broadcast to subscribers."""
        step_run_id = params.get("step_run_id")
        text = params.get("text")

        if not step_run_id or text is None:
            return error("Missing 'step_run_id' or 'text' parameter")

        await self._broadcast(
            Event(
                "output.line",
                {
                    "session_id": step_run_id,
                    "text": text,
                    "timestamp": datetime.now().isoformat(),
                },
            )
        )
        return success({})

    async def _handle_manager_status(self) -> Response:
        """Return manager status."""
        return success(self.manager.get_status())

    async def _handle_manager_acquire(self, params: dict) -> Response:
        """Try to acquire a manager slot."""
        run_id = params.get("run_id")
        if not run_id:
            return error("Missing 'run_id' parameter")

        acquired, reason = self.manager.acquire(run_id)
        if acquired:
            await self._broadcast(
                Event(
                    "scheduler.slot.acquired",
                    {
                        "run_id": run_id,
                        "slots_used": self.manager.slots_used(),
                    },
                )
            )
        return success(
            {
                "acquired": acquired,
                "reason": reason,
                "slots_used": self.manager.slots_used(),
            }
        )

    async def _handle_manager_release(self, params: dict) -> Response:
        """Release a manager slot."""
        run_id = params.get("run_id")
        if not run_id:
            return error("Missing 'run_id' parameter")

        self.manager.release(run_id)
        await self._broadcast(
            Event(
                "scheduler.slot.released",
                {
                    "run_id": run_id,
                    "slots_used": self.manager.slots_used(),
                },
            )
        )
        return success({"slots_used": self.manager.slots_used()})

    async def _handle_worktrees_list(self, params: dict) -> Response:
        """Return worktree list with staleness and recent steps."""
        repo = params.get("repo")
        if not repo:
            return error("Missing 'repo' parameter")

        repo_path = Path(repo)
        if not repo_path.exists():
            return error(f"Repository not found: {repo}")

        # Auto-install git hooks if not present
        try:
            status = hooks_status(repo_path)
            if not all(status.values()):
                install_hooks(repo_path)
        except Exception:
            pass  # Don't fail worktree list if hook install fails

        try:
            service = get_worktree_state_service()
            worktrees = service.list_worktrees(repo_path)
            return success({"worktrees": worktrees})
        except Exception as e:
            return error(f"Failed to list worktrees: {e}")

    async def _handle_worktrees_changed(self, params: dict) -> Response:
        """Handle notification that a worktree changed.

        Called by CLI commands (wt create, lf run, etc.) to emit rich events.
        """
        repo = params.get("repo")
        branch = params.get("branch")
        reason = params.get("reason", "changed")

        if not repo or not branch:
            return error("Missing 'repo' or 'branch' parameter")

        repo_path = Path(repo)
        service = get_worktree_state_service()
        service.invalidate(repo_path)

        # Build event with full worktree status for in-place UI updates
        data: dict = {"branch": branch, "reason": reason, "repo": str(repo_path)}
        worktree_status = service.get_one(repo_path, branch)
        if worktree_status:
            data["worktree"] = worktree_status

        await self._broadcast(Event("worktree.updated", data))
        return success({"branch": branch, "reason": reason})

    async def _broadcast(self, event: Event) -> None:
        metrics.increment("events_broadcast")
        message = (event.serialize() + "\n").encode()
        for writer, patterns in list(self.subscriptions.items()):
            if any(fnmatch.fnmatch(event.event, p) for p in patterns):
                try:
                    writer.write(message)
                    await writer.drain()
                except Exception:
                    self.clients.discard(writer)
                    self.subscriptions.pop(writer, None)

    async def _run_pr_poller(self) -> None:
        """Background task for PR state polling."""
        # Initialize: scan for existing open PRs to track
        await self._init_pr_tracking()

        service = get_worktree_state_service()

        def get_worktree(repo: Path, branch: str) -> dict | None:
            service.invalidate(repo)
            return service.get_one(repo, branch)

        await self.pr_poller.run(self._broadcast, get_worktree)

    async def _init_pr_tracking(self) -> None:
        """Scan existing worktrees and track any with open PRs."""
        from loopflow.lfd.autoprune import get_repos_to_check

        service = get_worktree_state_service()
        for repo in get_repos_to_check():
            try:
                worktrees = service.list_worktrees(repo)
                for wt in worktrees:
                    ci_info = wt.get("ci", {})
                    pr_url = ci_info.get("url")
                    pr_state = ci_info.get("state")

                    # Track open PRs for polling
                    if pr_url and pr_state and pr_state.upper() == "OPEN":
                        pr_number = self._extract_pr_number(pr_url)
                        if pr_number:
                            branch = wt.get("branch")
                            if branch:
                                self.pr_poller.track(repo, branch, pr_number)
            except Exception:
                pass  # Don't fail startup if one repo has issues

    def _extract_pr_number(self, pr_url: str | None) -> int | None:
        """Extract PR number from URL like https://github.com/org/repo/pull/123."""
        if not pr_url:
            return None
        try:
            return int(pr_url.rstrip("/").split("/")[-1])
        except (ValueError, IndexError):
            return None

    async def _periodic_check(self) -> None:
        """Periodically update dead processes and check wave stimuli."""
        from loopflow.lfd.autoprune import AutopruneManager, get_repos_to_check
        from loopflow.lfd.draft_prs import run_draft_pr_check

        autoprune_manager = AutopruneManager()

        while self._running:
            try:
                await asyncio.sleep(30)
                update_dead_processes()
                cleanup_stale_runs()

                # Check watch stimulus waves (file changes on main)
                activated_watch = run_watch_check()
                for wave_id in activated_watch:
                    await self._broadcast(
                        Event(
                            "wave.activated",
                            {"wave_id": wave_id, "stimulus": "watch"},
                        )
                    )

                # Check cron stimulus waves
                activated_cron = run_cron_check()
                for wave_id in activated_cron:
                    await self._broadcast(
                        Event(
                            "wave.activated",
                            {"wave_id": wave_id, "stimulus": "cron"},
                        )
                    )

                # Auto-create draft PRs for pushed branches
                worktree_service = get_worktree_state_service()
                for repo in get_repos_to_check():
                    created_prs = run_draft_pr_check(repo)
                    for branch in created_prs:
                        # Include full worktree status for in-place UI updates
                        worktree_service.invalidate(repo)  # Refresh to get new PR info
                        worktree_status = worktree_service.get_one(repo, branch)

                        # Track PR for CI polling
                        pr_url = (
                            worktree_status.get("ci", {}).get("url") if worktree_status else None
                        )
                        pr_number = self._extract_pr_number(pr_url)
                        if pr_number:
                            self.pr_poller.track(repo, branch, pr_number)

                        await self._broadcast(
                            Event(
                                "worktree.updated",
                                {
                                    "branch": branch,
                                    "reason": "draft_pr_created",
                                    "repo": str(repo),
                                    "worktree": worktree_status,
                                },
                            )
                        )

                # Auto-prune merged worktrees
                for repo in get_repos_to_check():
                    pruned = autoprune_manager.check_and_prune(repo)
                    for branch in pruned:
                        # Stop tracking PR for pruned worktrees
                        self.pr_poller.untrack(repo, branch)

                        await self._broadcast(
                            Event(
                                "worktree.pruned",
                                {
                                    "branch": branch,
                                    "repo": str(repo),
                                },
                            )
                        )

            except asyncio.CancelledError:
                break
            except Exception:
                pass


def _check_already_running(http_port: int = 8765) -> None:
    """Check if another lfd instance is running. Raises SystemExit if so."""
    import os
    import urllib.request

    try:
        req = urllib.request.Request(f"http://127.0.0.1:{http_port}/health", method="GET")
        with urllib.request.urlopen(req, timeout=2) as resp:
            import json

            data = json.loads(resp.read().decode())
            if data.get("ok"):
                result = data.get("result", {})
                pid = result.get("pid", "?")
                version = result.get("version", "?")
                # Don't fail if it's us (same PID)
                if pid != os.getpid():
                    raise SystemExit(
                        f"Another lfd instance is already running (pid {pid}, v{version}).\n"
                        f"Stop it with: kill {pid}"
                    )
    except urllib.error.URLError:
        # No server running, good
        pass
    except SystemExit:
        raise
    except Exception:
        # Some other error, probably no server running
        pass


async def run_server(socket_path: Path, http_port: int = 8765, grpc_port: int = 50051) -> None:
    """Main daemon entry point. Runs until terminated."""
    import logging
    import sqlite3

    from loopflow.lfd.daemon.grpc_server import start_grpc_server
    from loopflow.lfd.daemon.http_server import start_http_server
    from loopflow.lfd.daemon.launchd import remove_pid, write_pid
    from loopflow.lfd.db import DB_PATH

    # Enforce single instance
    _check_already_running(http_port)

    logging.basicConfig(level=logging.INFO, format="%(message)s")
    logger = logging.getLogger("lfd")
    logger.info(f"lfd starting (socket={socket_path}, http={http_port}, grpc={grpc_port})")
    server = Server(socket_path)
    http_server = None
    grpc_server = None

    # Write PID file for process tracking
    write_pid()

    async def shutdown():
        logger.info("Shutting down gracefully...")

        # Stop accepting new connections
        await server.stop()
        if http_server:
            await http_server.stop()
        if grpc_server:
            await grpc_server.stop()

        # Checkpoint WAL to ensure all data is persisted
        try:
            if DB_PATH.exists():
                conn = sqlite3.connect(DB_PATH)
                conn.execute("PRAGMA wal_checkpoint(TRUNCATE)")
                conn.close()
                logger.info("WAL checkpoint completed")
        except Exception as e:
            logger.warning(f"WAL checkpoint failed: {e}")

        remove_pid()
        logger.info("Shutdown complete")

    loop = asyncio.get_event_loop()
    for sig in (signal.SIGTERM, signal.SIGINT):
        loop.add_signal_handler(sig, lambda: asyncio.create_task(shutdown()))

    try:
        # Start HTTP server alongside socket server
        http_server = await start_http_server(http_port)

        # Start gRPC server on separate port
        grpc_server = await start_grpc_server(
            port=grpc_port,
            manager=server.manager,
            broadcast_callback=server._broadcast,
        )
        logger.info(f"lfd ready (http={http_port}, grpc={grpc_port})")

        await server.start()
    finally:
        if http_server:
            await http_server.stop()
        if grpc_server:
            await grpc_server.stop()
        remove_pid()
