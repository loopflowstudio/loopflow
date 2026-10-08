# /// script
# requires-python = ">=3.11"
# dependencies = ["websockets>=15,<16"]
# ///
"""Inspect captured Codex skill paths against a credential-free local API."""

import argparse
import asyncio
import json
import os
import shutil
import signal
import tempfile
import threading
import uuid
from contextlib import AsyncExitStack, asynccontextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from continuity import _request, _send, _until
from request_mapping import _Terminal
from websockets.asyncio.client import unix_connect
from websockets.asyncio.server import unix_serve


async def _turn(process: asyncio.subprocess.Process, workspace: Path, inputs: list[dict]) -> None:
    thread = await _request(
        process,
        "thread/start",
        {
            "cwd": str(workspace),
            "approvalPolicy": "never",
            "sandbox": "read-only",
        },
    )
    await _request(process, "turn/start", {"threadId": thread["thread"]["id"], "input": inputs})
    await _until(process, lambda event: event.get("method") == "turn/completed")


@asynccontextmanager
async def _app_server(
    codex: str, workspace: Path, env: dict[str, str], endpoint: Path | None = None
):
    process = await asyncio.create_subprocess_exec(
        codex,
        "app-server",
        "--listen",
        f"unix://{endpoint}" if endpoint else "stdio://",
        cwd=workspace,
        env=env,
        stdin=asyncio.subprocess.PIPE,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.DEVNULL,
        start_new_session=True,
    )
    try:
        if endpoint:
            async with asyncio.timeout(15):
                while not endpoint.exists():
                    if process.returncode is not None:
                        raise RuntimeError("fixture engine exited before opening its socket")
                    await asyncio.sleep(0.02)
        else:
            await _request(
                process,
                "initialize",
                {
                    "clientInfo": {"name": "lf_skill_fixture", "version": "1"},
                    "capabilities": {"experimentalApi": True},
                },
            )
            await _send(process, {"method": "initialized"})
        yield process
    finally:
        process.stdin.close()
        if endpoint and process.returncode is None:
            os.killpg(process.pid, signal.SIGTERM)
        try:
            await asyncio.wait_for(process.wait(), 5)
        except TimeoutError:
            os.killpg(process.pid, signal.SIGKILL)
            await process.wait()


async def _boundary_recovery(
    codex: str,
    workspace: Path,
    env: dict[str, str],
    source: Path,
    context: str,
    delivery: tuple[threading.Event, threading.Event],
    queued: bool = False,
) -> dict:
    endpoint = workspace.parent / "engine.sock"
    checks = {}
    retained = []
    async with _app_server(codex, workspace, env, endpoint):
        for interrupt in [False] if queued else [False, True]:
            case, thread_id, capture = await _boundary_delivery(
                endpoint, workspace, source, context, delivery, interrupt, codex, env, queued
            )
            status = "interrupted" if interrupt else "completed"
            checks.update({f"{status}_{key}": value for key, value in case.items()})
            retained.append((status, thread_id, capture))
    # A fresh engine must recover the same identities and outcomes from disk.
    async with _app_server(codex, workspace, env) as process:
        for status, thread_id, capture in retained:
            history = await _request(
                process, "thread/read", {"threadId": thread_id, "includeTurns": True}
            )
            receipts = _invocation_receipts(history["thread"], capture)
            checks[f"{status}_identity_survives_engine_restart"] = len(receipts) == 1
            checks[f"{status}_outcome_survives_engine_restart"] = (
                len(receipts) == 1 and receipts[0][0]["status"] == status
            )
    return checks


async def _catalog_probe(
    process: asyncio.subprocess.Process,
    workspace: Path,
    snapshot: Path,
    context: str,
    additive: str | None,
    source: Path,
) -> dict:
    roots = snapshot.parent.parent
    listed = await _request(
        process,
        "skills/list",
        {
            "cwds": [str(workspace)],
            "forceReload": True,
            "perCwdExtraUserRoots": [{"cwd": str(workspace), "extraUserRoots": [str(roots)]}],
        },
    )
    ignored = str(snapshot) not in json.dumps(listed)
    for register in [False, True]:
        if register:
            await _request(process, "skills/extraRoots/set", {"extraRoots": [str(roots)]})
        await _turn(
            process,
            workspace,
            [
                {"type": "text", "text": context},
                {"type": "skill", "name": "audit", "path": str(snapshot)},
                {"type": "text", "text": "$audit alpha"},
            ],
        )
    await _request(process, "skills/extraRoots/set", {"extraRoots": []})
    cleared = await _request(
        process, "skills/list", {"cwds": [str(workspace)], "forceReload": True}
    )
    checks = {
        "documented_roots_ignored": ignored,
        "replacement_clears_root": str(snapshot) not in json.dumps(cleared),
    }
    if additive:
        sibling = snapshot.parents[3] / "sibling-skills/sibling/SKILL.md"
        sibling.parent.mkdir(parents=True)
        sibling.write_text(
            "---\nname: sibling\ndescription: Retained sibling skill.\n---\nKeep this skill.\n"
        )
        await _request(
            process, "skills/extraRoots/set", {"extraRoots": [str(sibling.parent.parent)]}
        )
        await _turn(process, workspace, [{"type": "text", "text": "$audit baseline"}])
        alias = f"lf-{uuid.uuid4().hex}"
        if additive == "alias":
            snapshot.write_text(
                snapshot.read_text().replace("name: audit\n", f"name: {alias}\n", 1)
            )
        mount = workspace / ".agents/skills" / alias
        mount.parent.mkdir(parents=True, exist_ok=True)
        mount.symlink_to(snapshot.parent, target_is_directory=True)
        listed = await _request(
            process, "skills/list", {"cwds": [str(workspace)], "forceReload": True}
        )
        skills = [skill for group in listed["data"] for skill in group["skills"]]
        selected = next(
            (skill for skill in skills if Path(skill["path"]).resolve() == snapshot), None
        )
        checks["additive_catalog_membership"] = selected is not None
        checks["sibling_root_preserved"] = any(
            Path(skill["path"]).resolve() == sibling for skill in skills
        )
        checks["original_skill_preserved"] = any(Path(skill["path"]) == source for skill in skills)
        if selected:
            await _turn(
                process,
                workspace,
                [
                    {"type": "skill", "name": selected["name"], "path": selected["path"]},
                    {"type": "text", "text": f"${selected['name']} additive"},
                ],
            )
        await _turn(process, workspace, [{"type": "text", "text": "$audit after"}])
    return checks


async def _ws_until(socket, matches) -> dict:
    async with asyncio.timeout(20):
        async for message in socket:
            event = json.loads(message)
            if matches(event):
                return event
            if "method" in event and "id" in event:
                raise RuntimeError(f"unexpected provider request: {event['method']}")
    raise RuntimeError("fixture connection ended before the expected event")


async def _ws_request(socket, method: str, params: dict, ident: str | None = None) -> dict:
    ident = ident or uuid.uuid4().hex
    await socket.send(json.dumps({"id": ident, "method": method, "params": params}))
    reply = await _ws_until(socket, lambda event: event.get("id") == ident)
    if "error" in reply:
        raise RuntimeError(f"{method}: {reply['error'].get('message')}")
    return reply["result"]


async def _read_thread(socket, thread_id: str) -> dict:
    history = await _ws_request(
        socket, "thread/read", {"threadId": thread_id, "includeTurns": True}
    )
    return history["thread"]


async def _initialize_socket(socket) -> None:
    await _ws_request(
        socket,
        "initialize",
        {
            "clientInfo": {"name": "lf_boundary_fixture", "version": "1"},
            "capabilities": {"experimentalApi": True},
        },
    )
    await socket.send(json.dumps({"method": "initialized"}))


def _invocation_receipts(thread: dict, capture: str) -> list[tuple[dict, dict]]:
    return [
        (turn, item)
        for turn in thread["turns"]
        for item in turn["items"]
        if item["type"] == "userMessage" and item.get("clientId") == capture
    ]


async def _boundary_delivery(
    endpoint: Path,
    workspace: Path,
    source: Path,
    context: str,
    delivery: tuple[threading.Event, threading.Event],
    interrupt: bool,
    codex: str,
    env: dict[str, str],
    queued: bool = False,
) -> tuple[dict, str, str]:
    received, release = delivery
    received.clear()
    release.clear()
    dropped = asyncio.Event()
    terminal_ready = asyncio.Event()
    lost_id = uuid.uuid4().hex
    capture = f"run_{uuid.uuid4().hex}"
    dropped_count = 0
    queue_starts = 0

    async def relay(client) -> None:
        nonlocal dropped_count
        async with unix_connect(str(endpoint), uri="ws://localhost") as upstream:
            resume_id = None

            async def forward_inputs() -> None:
                nonlocal resume_id, queue_starts
                async for message in client:
                    rpc = json.loads(message)
                    if rpc.get("method") == "thread/resume":
                        resume_id = rpc["id"]
                    if rpc.get("method") == "thread/queue/start":
                        queue_starts += 1
                    await upstream.send(message)

            async def forward_outputs() -> None:
                nonlocal dropped_count
                async for message in upstream:
                    rpc = json.loads(message)
                    if resume_id is not None and rpc.get("id") == resume_id and "result" in rpc:
                        terminal_ready.set()
                    if rpc.get("id") == lost_id:
                        # Fault injection: no reply bytes reach the invoking client.
                        dropped_count += 1
                        dropped.set()
                        continue
                    await client.send(message)

            tasks = [asyncio.create_task(forward_inputs()), asyncio.create_task(forward_outputs())]
            try:
                await asyncio.wait(tasks, return_when=asyncio.FIRST_COMPLETED)
            finally:
                for task in tasks:
                    task.cancel()
                await asyncio.gather(*tasks, return_exceptions=True)

    proxy = workspace.parent / "loss.sock"
    terminal = None
    draft = "UNSUBMITTED_DRAFT_" + uuid.uuid4().hex
    checks = {}

    async def close_terminal() -> None:
        checks["terminal_exit_zero"] = await asyncio.to_thread(terminal.close)
        if not checks["terminal_exit_zero"]:
            print(
                json.dumps({"terminal_tail": terminal.output[-5000:].decode(errors="replace")}),
                flush=True,
            )

    async with unix_serve(relay, str(proxy)), AsyncExitStack() as cleanup:
        async with unix_connect(str(proxy), uri="ws://localhost") as owner:
            await _initialize_socket(owner)
            thread = await _ws_request(
                owner,
                "thread/start",
                {"cwd": str(workspace), "approvalPolicy": "never", "sandbox": "read-only"},
            )
            thread_id = thread["thread"]["id"]
            await _ws_request(
                owner,
                "turn/start",
                {
                    "threadId": thread_id,
                    "input": [{"type": "text", "text": "Complete the baseline turn."}],
                },
            )
            await _ws_until(owner, lambda event: event.get("method") == "turn/completed")
            before = await _read_thread(owner, thread_id)
            if not interrupt:
                terminal = _Terminal(
                    [
                        codex,
                        "resume",
                        "--remote",
                        f"unix://{proxy}",
                        "--no-alt-screen",
                        thread_id,
                    ],
                    workspace,
                    env,
                )
                cleanup.push_async_callback(close_terminal)
                async with asyncio.timeout(15):
                    while not terminal_ready.is_set():
                        await asyncio.to_thread(terminal.pump, 0.1)
                await asyncio.to_thread(terminal.pump, 0.5)
                terminal.write(draft.encode())
                await asyncio.to_thread(terminal.pump, 0.5)
            if queued:
                competitor = await cleanup.enter_async_context(
                    unix_connect(str(endpoint), uri="ws://localhost")
                )
                await _initialize_socket(competitor)
                sibling = await _ws_request(
                    competitor,
                    "thread/start",
                    {
                        "cwd": str(workspace),
                        "approvalPolicy": "never",
                        "sandbox": "read-only",
                    },
                )
                sibling_id = sibling["thread"]["id"]
                await _ws_request(
                    competitor,
                    "turn/start",
                    {
                        "threadId": sibling_id,
                        "input": [{"type": "text", "text": "Preserve this sibling's work."}],
                    },
                )
                await _ws_until(competitor, lambda event: event.get("method") == "turn/completed")
                sibling_before = await _read_thread(competitor, sibling_id)
                await _ws_request(competitor, "thread/resume", {"threadId": thread_id})
                await _ws_request(
                    competitor,
                    "turn/start",
                    {
                        "threadId": thread_id,
                        "input": [{"type": "text", "text": "Start competing turn."}],
                    },
                )
                if not await asyncio.to_thread(received.wait, 15):
                    raise RuntimeError("fake API did not receive the competing turn")
                active_before = await _read_thread(competitor, thread_id)
            pending = asyncio.create_task(
                _ws_request(
                    owner,
                    "thread/queue/add" if queued else "turn/start",
                    {
                        "threadId": thread_id,
                        "clientUserMessageId": capture,
                        "input": [
                            {"type": "skill", "name": "audit", "path": str(source)},
                            {"type": "text", "text": "$audit alpha"},
                            {"type": "text", "text": context},
                        ],
                    },
                    lost_id,
                )
            )
            try:
                await asyncio.wait_for(dropped.wait(), 15)
                if not queued and not await asyncio.to_thread(received.wait, 15):
                    raise RuntimeError("fake API did not receive the skill turn")
            finally:
                pending.cancel()
                await asyncio.gather(pending, return_exceptions=True)
        # A different connection inherits only the retained invocation and thread.
        # The engine is still running; the lost reply's turn id is unavailable.
        async with unix_connect(str(endpoint), uri="ws://localhost") as successor:
            await _initialize_socket(successor)
            history = await _read_thread(successor, thread_id)
            matches = _invocation_receipts(history, capture)
            if queued:
                queue = await _ws_request(successor, "thread/queue/list", {"threadId": thread_id})
                submissions = [
                    item for item in queue["data"] if item["clientUserMessageId"] == capture
                ]
                checks.update(
                    queued_once=len(submissions) == 1,
                    not_in_active_turn=matches == [],
                    active_turn_unchanged=history == active_before,
                    sibling_unchanged=await _read_thread(successor, sibling_id) == sibling_before,
                )
                if len(submissions) != 1:
                    raise RuntimeError("lost queue acknowledgement did not retain one submission")
                submission = submissions[0]
                # Ask the native owner while the already-attached writer is active.
                # Its refusal must retain both the queue and current turn unchanged.
                ident = uuid.uuid4().hex
                await successor.send(
                    json.dumps(
                        {
                            "id": ident,
                            "method": "thread/queue/start",
                            "params": {
                                "threadId": thread_id,
                                "queuedSubmissionId": submission["id"],
                            },
                        }
                    )
                )
                reply = await _ws_until(successor, lambda event: event.get("id") == ident)
                checks["busy_start_rejected"] = (
                    reply.get("error", {}).get("message")
                    == "thread already has an active or pending turn"
                )
                checks["busy_start_preserves_active_turn"] = (
                    await _read_thread(successor, thread_id) == active_before
                )
                retained = await _ws_request(
                    successor, "thread/queue/list", {"threadId": thread_id}
                )
                checks["busy_start_preserves_queue"] = retained["data"] == queue["data"]
                release.set()
                async with asyncio.timeout(30):
                    while True:
                        await asyncio.to_thread(terminal.pump, 0.1)
                        history = await _read_thread(successor, thread_id)
                        matches = _invocation_receipts(history, capture)
                        if len(matches) == 1 and matches[0][0]["status"] == "completed":
                            break
                retained = await _ws_request(
                    successor, "thread/queue/list", {"threadId": thread_id}
                )
                checks.update(
                    queue_consumed=retained["data"] == [],
                    queued_input_unchanged=matches[0][1]["content"] == submission["input"],
                    competing_turn_preserved=history["turns"][1]["id"]
                    == active_before["turns"][1]["id"]
                    and history["turns"][1]["status"] == "completed"
                    and history["turns"][1]["items"][: len(active_before["turns"][1]["items"])]
                    == active_before["turns"][1]["items"],
                    skill_on_separate_turn=matches[0][0]["id"] != active_before["turns"][1]["id"],
                )
            content = matches[0][1]["content"] if len(matches) == 1 else []
            checks.update(
                {
                    "reply_dropped": dropped_count == 1,
                    "caller_cancelled": pending.cancelled(),
                    "same_thread": history["id"] == thread_id,
                    "one_native_invocation": len(matches) == 1,
                    "skill_receipt": any(
                        item.get("type") == "skill" and item.get("path") == str(source)
                        for item in content
                    ),
                    "exact_arguments": any(item.get("text") == "$audit alpha" for item in content),
                    "separate_context": any(item.get("text") == context for item in content),
                    "prior_turn_unchanged": history["turns"][0] == before["turns"][0],
                }
            )
            print(json.dumps({"boundary_recovery": checks}), flush=True)
            if interrupt and len(matches) == 1:
                await _ws_request(
                    successor,
                    "turn/interrupt",
                    {
                        "threadId": thread_id,
                        "turnId": matches[0][0]["id"],
                    },
                )
            release.set()
            expected_status = "interrupted" if interrupt else "completed"
            async with asyncio.timeout(15):
                while True:
                    history = await _read_thread(successor, thread_id)
                    matches = _invocation_receipts(history, capture)
                    if len(matches) == 1 and matches[0][0]["status"] == expected_status:
                        break
                    await asyncio.sleep(0.02)
            checks["outcome_without_resubmission"] = True
            checks["expected_turns"] = len(history["turns"]) == (3 if queued else 2)
            if terminal:
                try:
                    await asyncio.to_thread(terminal.pump, 0.5)
                    terminal.write(b"\r")
                    async with asyncio.timeout(15):
                        while True:
                            await asyncio.to_thread(terminal.pump, 0.1)
                            history = await _read_thread(successor, thread_id)
                            turns = history["turns"]
                            if (
                                len(turns) == (4 if queued else 3)
                                and turns[-1]["status"] == "completed"
                            ):
                                break
                    checks["terminal_draft_preserved"] = any(
                        item["type"] == "userMessage"
                        and any(block.get("text") == draft for block in item["content"])
                        for item in turns[-1]["items"]
                    )
                    if queued:
                        checks["queue_consumed_without_client_start"] = queue_starts == 0
                        checks["sibling_unchanged_after_delivery"] = (
                            await _read_thread(successor, sibling_id) == sibling_before
                        )
                finally:
                    await cleanup.aclose()
            print(
                json.dumps(
                    {"boundary_case": "interrupted" if interrupt else "completed", "checks": checks}
                ),
                flush=True,
            )
            return checks, thread_id, capture


async def _steer_redelivery(
    process: asyncio.subprocess.Process,
    workspace: Path,
    source: Path,
    marker: str,
    delivery: tuple[threading.Event, threading.Event],
) -> dict:
    received, release = delivery
    inputs = [
        {"type": "skill", "name": "audit", "path": str(source)},
        {"type": "text", "text": f"$audit {marker}"},
    ]
    checks = {}
    for count in [2, 1]:
        received.clear()
        release.clear()
        thread = await _request(
            process,
            "thread/start",
            {"cwd": str(workspace), "approvalPolicy": "never", "sandbox": "read-only"},
        )
        thread_id = thread["thread"]["id"]
        turn = await _request(
            process,
            "turn/start",
            {"threadId": thread_id, "input": [{"type": "text", "text": "Start the fixture."}]},
        )
        if not await asyncio.to_thread(received.wait, 15):
            raise RuntimeError("fake API did not receive the initial turn")
        request_id = uuid.uuid4().hex
        request = {
            "id": request_id,
            "method": "turn/steer",
            "params": {
                "threadId": thread_id,
                "expectedTurnId": turn["turn"]["id"],
                "input": inputs,
            },
        }
        replies = []
        try:
            for _ in range(count):
                await _send(process, request)
                # Keep replies as evidence; a caller losing the first reply
                # would resend these exact bytes with the same RPC id.
                replies.append(await _until(process, lambda event: event.get("id") == request_id))
        finally:
            release.set()
        await _until(process, lambda event: event.get("method") == "turn/completed")
        history = await _request(
            process, "thread/read", {"threadId": thread_id, "includeTurns": True}
        )
        case = "duplicate" if count == 2 else "single"
        checks[f"{case}_accepted"] = all(
            reply.get("result", {}).get("turnId") == turn["turn"]["id"] for reply in replies
        )
        checks[f"{case}_thread_retained"] = history["thread"]["id"] == thread_id
    # A fresh turn establishes that the same definition and input are valid.
    await _turn(process, workspace, inputs)
    return checks


async def _boundary_race(
    codex: str,
    workspace: Path,
    env: dict[str, str],
    source: Path,
    context: str,
    delivery: tuple[threading.Event, threading.Event],
) -> dict:
    received, release = delivery
    endpoint = workspace.parent / "race.sock"
    async with _app_server(codex, workspace, env, endpoint):
        async with (
            unix_connect(str(endpoint), uri="ws://localhost") as owner,
            unix_connect(str(endpoint), uri="ws://localhost") as competitor,
        ):
            await _initialize_socket(owner)
            await _initialize_socket(competitor)
            thread = await _ws_request(
                owner,
                "thread/start",
                {"cwd": str(workspace), "approvalPolicy": "never", "sandbox": "read-only"},
            )
            thread_id = thread["thread"]["id"]
            await _ws_request(
                owner,
                "turn/start",
                {
                    "threadId": thread_id,
                    "input": [{"type": "text", "text": "Complete baseline."}],
                },
            )
            await _ws_until(owner, lambda event: event.get("method") == "turn/completed")
            idle = await _read_thread(owner, thread_id)
            await _ws_request(competitor, "thread/resume", {"threadId": thread_id})
            competing = await _ws_request(
                competitor,
                "turn/start",
                {
                    "threadId": thread_id,
                    "input": [{"type": "text", "text": "Start competing turn."}],
                },
            )
            if not await asyncio.to_thread(received.wait, 15):
                raise RuntimeError("fake API did not receive the competing turn")
            message_id = uuid.uuid4().hex
            inputs = [
                {"type": "skill", "name": "audit", "path": str(source)},
                {"type": "text", "text": f"$audit {context}"},
                {"type": "text", "text": "Separate context."},
            ]
            try:
                started = await _ws_request(
                    owner,
                    "turn/start",
                    {"threadId": thread_id, "clientUserMessageId": message_id, "input": inputs},
                )
            finally:
                release.set()
            await _ws_until(owner, lambda event: event.get("method") == "turn/completed")
            history = await _read_thread(owner, thread_id)
            matches = _invocation_receipts(history, message_id)
            checks = {
                "observed_idle": len(idle["turns"]) == 1
                and idle["turns"][0]["status"] == "completed",
                "start_joined_competing_turn": started["turn"]["id"] == competing["turn"]["id"],
                "one_correlated_receipt": len(matches) == 1,
                "receipt_on_competing_turn": len(matches) == 1
                and matches[0][0]["id"] == competing["turn"]["id"],
                "receipt_retains_skill": len(matches) == 1
                and inputs[0] in matches[0][1]["content"],
                "receipt_retains_arguments_and_context": len(matches) == 1
                and [item["text"] for item in matches[0][1]["content"] if item["type"] == "text"]
                == [inputs[1]["text"], inputs[2]["text"]],
            }
            # A new turn proves that the selected skill itself is valid.
            await _ws_request(owner, "turn/start", {"threadId": thread_id, "input": inputs})
            await _ws_until(owner, lambda event: event.get("method") == "turn/completed")
            return checks


def _skill_paths(request: dict) -> list[str]:
    return [
        block["text"].split("<path>", 1)[1].split("</path>", 1)[0]
        for item in request.get("input", [])
        if item.get("role") == "user"
        for block in item.get("content", [])
        if isinstance(block, dict)
        and block.get("text", "").startswith("<skill>\n")
        and "<path>" in block["text"]
        and "</path>" in block["text"]
    ]


def _last_user_texts(request: dict) -> list[str]:
    user = next(
        (item for item in reversed(request.get("input", [])) if item.get("role") == "user"),
        {},
    )
    return [block.get("text", "") for block in user.get("content", [])]


def _probe(codex: str, mode: str) -> bool:
    additive = mode if mode in ("original", "alias") else None
    requests = []
    received, release = threading.Event(), threading.Event()

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, format: str, *args: object) -> None:
            pass

        def handle(self) -> None:
            try:
                super().handle()
            except (BrokenPipeError, ConnectionResetError):
                if mode != "boundary":
                    raise
                # The interruption case deliberately closes its model stream.

        def do_POST(self) -> None:
            requests.append(json.loads(self.rfile.read(int(self.headers["Content-Length"]))))
            native_turn = any(
                text.startswith("<skill>\n") for text in _last_user_texts(requests[-1])
            )
            if (
                (mode == "redelivery" and len(requests) in (1, 3))
                or (mode == "boundary" and native_turn)
                or (
                    mode == "queue-race"
                    and _last_user_texts(requests[-1]) == ["Start competing turn."]
                )
                or (mode == "boundary-race" and len(requests) == 2)
            ):
                received.set()
                if not release.wait(20):
                    self.send_error(504, "steering probe did not release the fixture")
                    return
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.end_headers()
            item = {
                "type": "message",
                "id": "msg_fixture",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "Fixture complete."}],
            }
            response = {"id": "resp_fixture", "status": "in_progress", "output": []}
            self._event("response.created", {"response": response})
            self._event("response.output_item.added", {"output_index": 0, "item": item})
            self._event("response.output_item.done", {"output_index": 0, "item": item})
            response.update(
                status="completed",
                output=[item],
                usage={
                    "input_tokens": 20,
                    "output_tokens": 5,
                    "total_tokens": 25,
                    "input_tokens_details": {"cached_tokens": 0},
                    "output_tokens_details": {"reasoning_tokens": 0},
                },
            )
            self._event("response.completed", {"response": response})

        def _event(self, kind: str, value: dict) -> None:
            self.wfile.write(
                f"event: {kind}\ndata: {json.dumps({'type': kind, **value})}\n\n".encode()
            )
            self.wfile.flush()

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        with tempfile.TemporaryDirectory(prefix="lf-codex-skill-", dir="/tmp") as temporary:
            root = Path(temporary).resolve()
            home = root / "home"
            codex_home = home / ".codex"
            codex_home.mkdir(parents=True)
            (codex_home / "config.toml").write_text(
                'model = "gpt-5.4"\nmodel_provider = "fixture"\n'
                'cli_auth_credentials_store = "file"\nallow_login_shell = false\n'
                "[features]\nshell_snapshot = false\n"
                '[model_providers.fixture]\nname = "Local synthetic fixture"\n'
                f'base_url = "http://127.0.0.1:{server.server_port}/v1"\n'
                'wire_api = "responses"\nrequires_openai_auth = false\n'
                "[analytics]\nenabled = false\n[feedback]\nenabled = false\n"
            )
            workspace = root / "workspace"
            workspace.mkdir()
            skill = (home if additive else workspace) / ".agents/skills/audit/SKILL.md"
            skill.parent.mkdir(parents=True)
            marker, context = uuid.uuid4().hex, uuid.uuid4().hex
            source = (
                "---\nname: audit\ndescription: Local skill fixture.\n---\n"
                f"{marker}\nRead reference.txt.\n"
            )
            skill.write_text(source)
            (skill.parent / "reference.txt").write_text("bundled reference")
            env = {key: os.environ[key] for key in ("PATH", "TMPDIR", "LANG") if key in os.environ}
            env.update(HOME=str(home), CODEX_HOME=str(codex_home), LF_HOME=str(root / "lf"))
            snapshot = root / "captured/skills/audit/SKILL.md"
            snapshot.parent.mkdir(parents=True)
            snapshot.write_text(source)

            async def _native() -> dict:
                if mode == "boundary-race":
                    return await _boundary_race(
                        codex, workspace, env, skill, context, (received, release)
                    )
                if mode in ("boundary", "queue-race"):
                    return await _boundary_recovery(
                        codex,
                        workspace,
                        env,
                        skill,
                        context,
                        (received, release),
                        queued=mode == "queue-race",
                    )
                async with _app_server(codex, workspace, env) as process:
                    if mode == "redelivery":
                        return await _steer_redelivery(
                            process, workspace, skill, context, (received, release)
                        )
                    return await _catalog_probe(
                        process, workspace, snapshot, context, additive, skill
                    )

            checks = asyncio.run(_native())
            if mode == "boundary-race":
                checks.update(
                    expected_requests=len(requests) == 4,
                    raced_start_not_expanded=len(requests) >= 3 and not _skill_paths(requests[2]),
                    fresh_start_expanded=len(requests) == 4
                    and str(skill) in _skill_paths(requests[3]),
                )
                print(
                    json.dumps(
                        {"mode": "boundary-race", "requests": len(requests), "checks": checks}
                    ),
                    flush=True,
                )
                return all(checks.values())
            if mode in ("boundary", "queue-race"):
                expected_expansions = 1 if mode == "queue-race" else 2
                native_requests = [
                    request
                    for request in requests
                    if any(text.startswith("<skill>\n") for text in _last_user_texts(request))
                ]
                title_requests = [
                    request
                    for request in requests
                    if any(
                        text.startswith("Generate a concise, single-line task title")
                        for text in _last_user_texts(request)
                    )
                ]
                checks.update(
                    expected_conversation_requests=len(requests) - len(title_requests) == 5,
                    native_expansion=len(native_requests) == expected_expansions
                    and all(
                        _skill_paths(request) == [str(skill)]
                        and any(
                            f"<path>{skill}</path>" in text and marker in text
                            for text in _last_user_texts(request)
                            if text.startswith("<skill>\n")
                        )
                        for request in native_requests
                    ),
                    draft_not_submitted_with_skill=len(native_requests) == expected_expansions
                    and all(
                        "UNSUBMITTED_DRAFT_" not in json.dumps(request)
                        for request in native_requests
                    ),
                )
                print(
                    json.dumps(
                        {
                            "mode": mode,
                            "requests": len(requests),
                            "title_requests": len(title_requests),
                            "checks": checks,
                        }
                    ),
                    flush=True,
                )
                return all(checks.values())
            if mode == "redelivery":
                texts = (
                    [
                        block.get("text", "")
                        for item in requests[1].get("input", [])
                        if item.get("role") == "user"
                        for block in item.get("content", [])
                    ]
                    if len(requests) >= 2
                    else []
                )
                checks.update(
                    expected_requests=len(requests) == 5,
                    duplicate_model_input=sum(text.count(f"$audit {context}") for text in texts)
                    == 2,
                    steer_skill_not_expanded=len(requests) >= 2 and not _skill_paths(requests[1]),
                    single_steer_not_expanded=len(requests) >= 4
                    and not _skill_paths(requests[3])
                    and sum(
                        block.get("text", "").count(f"$audit {context}")
                        for item in requests[3].get("input", [])
                        if item.get("role") == "user"
                        for block in item.get("content", [])
                    )
                    == 1,
                    start_skill_expanded=len(requests) == 5
                    and str(skill) in _skill_paths(requests[4]),
                )
                print(
                    json.dumps(
                        {
                            "mode": "steer-redelivery",
                            "requests": len(requests),
                            "checks": checks,
                        }
                    ),
                    flush=True,
                )
                return all(checks.values())
            checks.update(
                expected_requests=len(requests) == (5 if additive else 2),
                unregistered_path_ignored=len(requests) >= 2
                and marker not in json.dumps(requests[0].get("input", [])),
                registered_path_expanded=len(requests) >= 2
                and any(
                    f"<path>{snapshot}</path>" in json.dumps(item) and marker in json.dumps(item)
                    for item in requests[1].get("input", [])
                    if item.get("role") == "user"
                ),
            )
            if additive:
                checks["additive_native_expansion"] = len(requests) == 5 and any(
                    marker in json.dumps(item)
                    and "<skill>" in json.dumps(item)
                    and f"<path>{skill}</path>" not in json.dumps(item)
                    for item in requests[3].get("input", [])
                    if item.get("role") == "user"
                )
                before, after = _skill_paths(requests[2]), _skill_paths(requests[-1])
                checks["implicit_selection_unchanged"] = before == after == [str(skill)]
                print(
                    json.dumps(
                        {
                            "before_mount": before,
                            "explicit_mount": _skill_paths(requests[3])
                            if len(requests) == 5
                            else [],
                            "after_mount": after,
                        }
                    ),
                    flush=True,
                )
            print(
                json.dumps(
                    {
                        "mode": "additive-catalog" if additive else "provider-counterexample",
                        "checks": checks,
                    }
                ),
                flush=True,
            )
            return all(checks.values())
    finally:
        release.set()
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.set_defaults(mode="catalog")
    mode = parser.add_mutually_exclusive_group()
    parser.add_argument("--codex", default=shutil.which("codex"))
    mode.add_argument(
        "--additive",
        dest="mode",
        choices=["original", "alias"],
        help="Probe a skill link while preserving sibling roots and plain invocation",
    )
    mode.add_argument(
        "--redelivery",
        dest="mode",
        action="store_const",
        const="redelivery",
        help="Check duplicate structured steering with an identical RPC id",
    )
    mode.add_argument(
        "--boundary-race",
        dest="mode",
        action="store_const",
        const="boundary-race",
        help="Start another client's turn between idle observation and native skill delivery",
    )
    mode.add_argument(
        "--queue-race",
        dest="mode",
        action="store_const",
        const="queue-race",
        help="Probe native queued submission against a competing client",
    )
    mode.add_argument(
        "--boundary",
        dest="mode",
        action="store_const",
        const="boundary",
        help="Lose a native turn reply, cancel its waiter and recover on a new connection",
    )
    args = parser.parse_args()
    if not args.codex:
        parser.error("codex is required")
    return 0 if _probe(args.codex, args.mode) else 1


if __name__ == "__main__":
    raise SystemExit(main())
