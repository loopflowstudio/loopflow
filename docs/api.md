---
layout: default
title: API Reference
---

# API Reference

The lfd daemon exposes a JSON-over-newline protocol on a Unix socket.

## Protocol

Connect to `~/.lf/lfd.sock`. Send JSON requests terminated by newline. Receive JSON responses terminated by newline.

### Request Format

```json
{
  "method": "method.name",
  "params": {},
  "id": "optional-request-id"
}
```

### Response Format

Success:

```json
{
  "ok": true,
  "result": { ... },
  "id": "request-id-if-provided"
}
```

Error:

```json
{
  "ok": false,
  "error": "Error message",
  "id": "request-id-if-provided"
}
```

### Event Format

After subscribing, events arrive as:

```json
{
  "event": "event.name",
  "data": { ... }
}
```

## Methods

### status

Returns daemon health information.

**Request:**
```json
{"method": "status", "params": {}}
```

**Response:**
```json
{
  "ok": true,
  "result": {
    "pid": 12345,
    "agents_defined": 3,
    "agents_running": 1,
    "sessions_active": 2
  }
}
```

### agents.list

Returns all agent definitions with runtime status.

**Request:**
```json
{"method": "agents.list", "params": {}}
```

**Response:**
```json
{
  "ok": true,
  "result": [
    {
      "name": "my-agent",
      "repo": "/path/to/repo",
      "pipeline": "ship",
      "trigger": {"kind": "manual"},
      "status": "running",
      "iteration": 5,
      "pid": 12346
    }
  ]
}
```

### agents.start

Start an agent by name.

**Request:**
```json
{"method": "agents.start", "params": {"name": "my-agent"}}
```

**Response:**
```json
{
  "ok": true,
  "result": {"name": "my-agent", "pid": 12346}
}
```

### agents.stop

Stop a running agent.

**Request:**
```json
{"method": "agents.stop", "params": {"name": "my-agent"}}
```

**Response:**
```json
{
  "ok": true,
  "result": {"name": "my-agent"}
}
```

### sessions.list

Returns active sessions.

**Request:**
```json
{"method": "sessions.list", "params": {}}
```

**Response:**
```json
{
  "ok": true,
  "result": [
    {
      "id": "uuid",
      "task": "implement",
      "repo": "/path/to/repo",
      "worktree": "/path/to/worktree",
      "status": "running",
      "started_at": "2025-01-14T10:30:00Z",
      "pid": 12347,
      "model": "claude-code",
      "run_mode": "auto"
    }
  ]
}
```

### sessions.history

Returns session history, optionally filtered.

**Request:**
```json
{"method": "sessions.history", "params": {"worktree": "/path/to/worktree", "limit": 20}}
```

**Parameters:**
- `worktree` (optional): Filter by worktree path
- `repo` (optional): Filter by repository path
- `limit` (optional): Max results (default: 20)

**Response:**
```json
{
  "ok": true,
  "result": [
    {
      "id": "uuid",
      "task": "review",
      "repo": "/path/to/repo",
      "worktree": "/path/to/worktree",
      "status": "completed",
      "started_at": "2025-01-14T10:00:00Z",
      "ended_at": "2025-01-14T10:15:00Z",
      "model": "claude-code",
      "run_mode": "auto"
    }
  ]
}
```

### sessions.start

Record a session start. Used internally by the task runner.

**Request:**
```json
{
  "method": "sessions.start",
  "params": {
    "session": {
      "id": "uuid",
      "task": "implement",
      "repo": "/path/to/repo",
      "worktree": "/path/to/worktree",
      "status": "running",
      "started_at": "2025-01-14T10:30:00Z",
      "pid": 12347,
      "model": "claude-code",
      "run_mode": "auto"
    }
  }
}
```

### sessions.end

Record a session end. Used internally by the task runner.

**Request:**
```json
{
  "method": "sessions.end",
  "params": {
    "session_id": "uuid",
    "status": "completed"
  }
}
```

### subscribe

Subscribe to events. After subscribing, the connection receives events matching the patterns.

**Request:**
```json
{"method": "subscribe", "params": {"events": ["session.*", "agent.*"]}}
```

**Response:**
```json
{
  "ok": true,
  "result": {"subscribed": ["session.*", "agent.*"]}
}
```

**Pattern matching:** Uses glob patterns. `*` matches any characters within a segment.

### notify

Broadcast a custom event to subscribers.

**Request:**
```json
{"method": "notify", "params": {"event": "worktree.created", "data": {"branch": "feature-x"}}}
```

**Response:**
```json
{
  "ok": true,
  "result": {"event": "worktree.created"}
}
```

## Events

Events are broadcast to subscribed clients.

| Event | Data | Description |
|-------|------|-------------|
| `session.started` | `{id, task}` | Task session began |
| `session.ended` | `{id, status}` | Task session completed |
| `agent.started` | `{name, pid}` | Agent started running |
| `agent.stopped` | `{name}` | Agent stopped |
| `worktree.created` | `{branch, path}` | New worktree created |
| `worktree.deleted` | `{branch}` | Worktree removed |

## Client Examples

### Python (async)

```python
import asyncio
import json

async def connect():
    reader, writer = await asyncio.open_unix_connection("~/.lf/lfd.sock")

    # Send request
    request = {"method": "status", "params": {}}
    writer.write((json.dumps(request) + "\n").encode())
    await writer.drain()

    # Read response
    line = await reader.readline()
    response = json.loads(line)
    print(response)

    writer.close()
    await writer.wait_closed()
```

### Swift (Network framework)

```swift
let endpoint = NWEndpoint.unix(path: "~/.lf/lfd.sock")
let connection = NWConnection(to: endpoint, using: NWParameters())
connection.start(queue: .main)

let request = "{\"method\":\"subscribe\",\"params\":{\"events\":[\"session.*\"]}}\n"
connection.send(content: request.data(using: .utf8), completion: .idempotent)
```

### Shell (netcat)

```bash
echo '{"method":"status","params":{}}' | nc -U ~/.lf/lfd.sock
```
