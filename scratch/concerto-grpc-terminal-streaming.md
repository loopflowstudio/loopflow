---
status: deferred
phase: 4
---

# gRPC Terminal Streaming

Bidirectional stream for remote terminal I/O.

## Status: Deferred

This design was superseded by the non-interactive mobile approach. See `scratch/concerto-mobile-direction.md`.

**Why deferred:**
- Terminal UX on phones is poor
- We support multiple agents (Claude Code, Codex, Gemini)—each has different terminal UI
- Building an iOS terminal renderer is significant work
- Mobile users want status and actions, not raw terminal

**If we build it:** Power-user escape hatch, not the primary mobile experience.

---

## Original Problem

Users want to manage waves from their phone while their Mac runs agents at home. Phase 2 registration gives us discovery (mobile can find the user's lfd), but there's no way to actually interact with a running terminal session.

Current limitations:
- `ConnectWave` spawns PTY but output is discarded (`sessions.rs:73`)
- `StreamOutput` is server→client only (can't send keystrokes)
- No tunnel infrastructure for NAT traversal
- No reconnection handling if mobile loses signal

Mobile users need the same interactive experience as local Ghostty: see output in real-time, type commands, resize terminal, and resume after disconnection.

## Approach

Add a bidirectional gRPC stream (`TerminalStream`) that multiplexes PTY I/O over the existing relay architecture. lfd maintains an outbound tunnel to loopflow.studio; mobile connects through the relay.

```
┌─────────────┐              ┌─────────────────┐              ┌─────────────────────────┐
│   Mobile    │    TLS       │  loopflow.studio│   tunnel     │   Mac/Server + lfd      │
│  Concerto   │ ◄──────────► │     (relay)     │ ◄──────────► │                         │
│             │              │                 │              │   ┌─────────────────┐   │
│  ┌───────┐  │   gRPC       │  terminates TLS │   gRPC       │   │  pty session    │   │
│  │ term  │  │   bidir      │  validates JWT  │   bidir      │   │  ┌───────────┐  │   │
│  │ view  │  │   stream     │  routes to lfd  │   stream     │   │  │ claude    │  │   │
│  └───────┘  │              │                 │              │   │  │ code      │  │   │
└─────────────┘              └─────────────────┘              │   │  └───────────┘  │   │
                                                              │   └─────────────────┘   │
                                                              └─────────────────────────┘
```

Key insight: the tunnel is just another gRPC stream. lfd opens `TerminalStream` to loopflow.studio as a "reverse" connection. When mobile connects, loopflow.studio bridges the two streams.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| WebSocket with separate protocol | Simpler client, browser-compatible | Doesn't integrate with existing gRPC infrastructure, extra protocol to maintain |
| SSH tunnel from lfd to relay | Battle-tested, rich terminal features | Requires SSH daemon config, key management, doesn't use existing auth |
| Direct connection with STUN/TURN | Lower latency when possible | Complex NAT traversal, unreliable, extra infrastructure |
| Mosh-style UDP with FEC | Better on flaky connections | Massive implementation complexity, doesn't solve NAT, unusual for mobile |

gRPC bidirectional stream wins: integrates with existing proto/auth/relay infrastructure, one stream handles both directions, built-in flow control, connection management handled by HTTP/2.

## Key decisions

### 1. Single bidirectional stream, not paired unidirectional

A single `TerminalStream` RPC handles both input and output:

```protobuf
rpc TerminalStream(stream TerminalInput) returns (stream TerminalOutput);
```

Alternatives:
- Paired streams: `SendInput(stream)` + `ReceiveOutput(stream)` - harder to correlate, connection lifecycle complexity
- Separate RPCs: `WriteTerminal(bytes)` + `ReadTerminal()` - loses streaming semantics, higher latency

Single bidirectional gives us: atomic session lifecycle, natural backpressure, simpler state management.

### 2. Session ID in first message, not metadata

First `TerminalInput` message must contain session ID:

```protobuf
message TerminalInput {
  oneof payload {
    SessionStart start = 1;    // First message: which session
    bytes data = 2;            // Subsequent: raw terminal input
    TerminalResize resize = 3; // Window size change
  }
}
```

Alternatives:
- gRPC metadata: Can't change mid-stream, harder to handle reconnection
- Separate `StartSession` RPC: Extra round trip, race conditions

First-message-is-session keeps everything in one stream while allowing reconnection to existing sessions.

### 3. Relay bridges streams, doesn't parse content

loopflow.studio acts as a dumb pipe: validates connection token once on connect, then forwards bytes between mobile and lfd. It doesn't parse terminal protocol.

This follows the registration architecture (per `roadmap/concerto/concerto-lfd-registration.md`): loopflow.studio handles auth and routing, lfd handles terminal semantics.

### 4. Ring buffer for reconnection, ~1000 lines

lfd maintains a ring buffer of recent terminal output (default: 100KB, ~1000 typical lines). On reconnection, client can request replay from a sequence number.

```protobuf
message SessionStart {
  string session_id = 1;
  optional uint64 replay_from_seq = 2;  // Resume from this sequence
}

message TerminalOutput {
  uint64 seq = 1;           // For replay coordination
  bytes data = 2;           // Raw terminal output
}
```

Tradeoffs:
- Buffer too small: user loses context on reconnection
- Buffer too large: memory pressure on lfd
- 100KB handles typical interactive sessions without stress

### 5. Session tied to agent, not wave

Each terminal session maps 1:1 to a running agent. When the agent exits, the session ends. Wave-level abstraction (`ConnectWave`) creates or finds the agent, then returns a session ID for `TerminalStream`.

This matches existing behavior: `ConnectWave` already returns `agent_id`. The new `session_id` is derived from agent state.

### 6. Tunnel uses same TerminalStream RPC in reverse

lfd doesn't open a special "tunnel" connection. Instead, when registered:

1. lfd opens `TerminalStream` to loopflow.studio as a listener
2. First message indicates "I'm lfd, waiting for connections"
3. loopflow.studio holds this stream
4. When mobile connects, loopflow.studio bridges the streams

This reuses the same proto messages and avoids a separate tunnel protocol. The "direction" is indicated in the first message.

## Scope

**In scope:**
- Proto definition for `TerminalStream` RPC
- lfd terminal session management with ring buffer
- PTY I/O multiplexing over gRPC stream
- Session lifecycle (start, reconnect, resize, end)
- Integration with existing `ConnectWave` flow
- loopflow.studio stream bridging (design only; implementation is server-side)

**Out of scope:**
- Mobile client terminal rendering (Phase 3 iOS work)
- Ghostty integration changes (local still uses Ghostty directly)
- Multi-session support (one session per agent)
- Recording/playback (future feature)
- Terminal protocol interpretation (ANSI, etc.) - it's opaque bytes

## Implementation

### Proto additions

```protobuf
// Add to control.proto

// Bidirectional terminal I/O stream
rpc TerminalStream(stream TerminalInput) returns (stream TerminalOutput);

message TerminalInput {
  oneof payload {
    SessionStart start = 1;
    bytes data = 2;
    TerminalResize resize = 3;
    SessionEnd end = 4;
  }
}

message SessionStart {
  string session_id = 1;
  optional uint64 replay_from_seq = 2;  // Resume point
  optional TerminalSize size = 3;       // Initial window size
}

message TerminalResize {
  TerminalSize size = 1;
}

message TerminalSize {
  uint32 rows = 1;
  uint32 cols = 2;
}

message SessionEnd {
  string session_id = 1;
}

message TerminalOutput {
  oneof payload {
    SessionStarted started = 1;
    bytes data = 2;
    SessionEnded ended = 3;
    SessionError error = 4;
  }
  uint64 seq = 10;  // Sequence for replay
}

message SessionStarted {
  string session_id = 1;
  uint64 current_seq = 2;  // Current sequence, for replay coordination
}

message SessionEnded {
  string session_id = 1;
  int32 exit_code = 2;
}

message SessionError {
  string code = 1;
  string message = 2;
}
```

### Terminal session manager

```rust
// rust/lfd/src/terminal.rs

pub struct TerminalSession {
    session_id: String,
    agent_id: String,
    pty_master: Box<dyn MasterPty + Send>,
    pty_writer: Box<dyn Write + Send>,
    output_buffer: RingBuffer,
    current_seq: AtomicU64,
    size: Mutex<TerminalSize>,
}

impl TerminalSession {
    pub fn spawn(agent_id: &str, command: PtyCommand, size: TerminalSize) -> Result<Self, SessionError> {
        // Create PTY, spawn command, set up I/O handles
    }

    pub fn write(&self, data: &[u8]) -> Result<(), SessionError> {
        // Write to PTY master
    }

    pub fn resize(&self, size: TerminalSize) -> Result<(), SessionError> {
        // Resize PTY
    }

    pub fn subscribe(&self, from_seq: Option<u64>) -> impl Stream<Item = (u64, Bytes)> {
        // Returns stream of (seq, data) starting from seq or current
        // Replays from ring buffer if from_seq < current_seq
    }
}

pub struct TerminalSessionManager {
    sessions: RwLock<HashMap<String, Arc<TerminalSession>>>,
}

impl TerminalSessionManager {
    pub fn create_or_get(&self, agent_id: &str, command: PtyCommand, size: TerminalSize)
        -> Result<Arc<TerminalSession>, SessionError> {
        // Create new session or return existing for agent
    }

    pub fn get(&self, session_id: &str) -> Option<Arc<TerminalSession>> {
        // Look up existing session
    }

    pub fn remove(&self, session_id: &str) {
        // Clean up on agent exit
    }
}
```

### Ring buffer for replay

```rust
// rust/lfd/src/terminal.rs

pub struct RingBuffer {
    buffer: VecDeque<(u64, Bytes)>,
    max_bytes: usize,
    current_bytes: usize,
}

impl RingBuffer {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            buffer: VecDeque::new(),
            max_bytes,
            current_bytes: 0,
        }
    }

    pub fn push(&mut self, seq: u64, data: Bytes) {
        self.current_bytes += data.len();
        self.buffer.push_back((seq, data));

        // Evict oldest until under limit
        while self.current_bytes > self.max_bytes && !self.buffer.is_empty() {
            if let Some((_, old)) = self.buffer.pop_front() {
                self.current_bytes -= old.len();
            }
        }
    }

    pub fn replay_from(&self, from_seq: u64) -> impl Iterator<Item = &(u64, Bytes)> {
        self.buffer.iter().filter(move |(seq, _)| *seq >= from_seq)
    }

    pub fn current_seq(&self) -> u64 {
        self.buffer.back().map(|(seq, _)| *seq).unwrap_or(0)
    }
}
```

### Server-side stream handler

```rust
// rust/lfd/src/server.rs

type TerminalInputStream = Pin<Box<dyn Stream<Item = Result<TerminalInput, Status>> + Send>>;
type TerminalOutputStream = Pin<Box<dyn Stream<Item = Result<TerminalOutput, Status>> + Send>>;

async fn terminal_stream(
    &self,
    request: Request<Streaming<TerminalInput>>,
) -> Result<Response<TerminalOutputStream>, Status> {
    self.check_auth(&request).await?;

    let mut input_stream = request.into_inner();

    // First message must be SessionStart
    let first = input_stream.next().await
        .ok_or_else(|| Status::invalid_argument("empty stream"))?
        .map_err(|e| Status::internal(e.to_string()))?;

    let start = match first.payload {
        Some(terminal_input::Payload::Start(s)) => s,
        _ => return Err(Status::invalid_argument("first message must be SessionStart")),
    };

    let session = self.terminal_sessions.get(&start.session_id)
        .ok_or_else(|| Status::not_found("session not found"))?;

    // Set initial size if provided
    if let Some(size) = start.size {
        session.resize(size.into())?;
    }

    // Get output subscription (with replay if requested)
    let output_sub = session.subscribe(start.replay_from_seq);

    // Spawn task to forward input to PTY
    let session_input = session.clone();
    tokio::spawn(async move {
        while let Some(Ok(msg)) = input_stream.next().await {
            match msg.payload {
                Some(terminal_input::Payload::Data(bytes)) => {
                    let _ = session_input.write(&bytes);
                }
                Some(terminal_input::Payload::Resize(r)) => {
                    let _ = session_input.resize(r.size.into());
                }
                Some(terminal_input::Payload::End(_)) => break,
                _ => {}
            }
        }
    });

    // Return output stream
    let output_stream = output_sub.map(|(seq, data)| {
        Ok(TerminalOutput {
            payload: Some(terminal_output::Payload::Data(data)),
            seq,
        })
    });

    // Prepend SessionStarted message
    let started = stream::once(async move {
        Ok(TerminalOutput {
            payload: Some(terminal_output::Payload::Started(SessionStarted {
                session_id: start.session_id,
                current_seq: session.current_seq(),
            })),
            seq: 0,
        })
    });

    Ok(Response::new(Box::pin(started.chain(output_stream))))
}
```

### Integration with ConnectWave

Modify `ConnectWave` to create a terminal session:

```rust
// Before: spawns PTY directly, output discarded
// After: creates TerminalSession, returns session_id

message ConnectWaveResponse {
  string worktree = 1;
  string step = 2;
  string agent_id = 3;
  string prompt_file = 4;
  optional string wave_run_id = 5;
  uint32 step_index = 6;
  string session_id = 7;  // NEW: for TerminalStream
}
```

Client flow:
1. Call `ConnectWave(wave_id)` → get `session_id`
2. Open `TerminalStream`, send `SessionStart { session_id }`
3. Stream I/O until agent exits or user disconnects

### Tunnel mode for relay

When lfd is registered with loopflow.studio, it maintains a "listener" stream:

```rust
// rust/lfd/src/tunnel.rs

pub async fn maintain_tunnel(
    config: &TunnelConfig,
    sessions: Arc<TerminalSessionManager>,
    cancel: CancellationToken,
) {
    loop {
        if cancel.is_cancelled() { break; }

        match open_tunnel_stream(config).await {
            Ok(stream) => {
                // This stream is held by loopflow.studio
                // When mobile connects, loopflow.studio sends tunnel commands
                if let Err(e) = handle_tunnel_stream(stream, sessions.clone()).await {
                    tracing::warn!("tunnel stream ended: {}", e);
                }
            }
            Err(e) => {
                tracing::warn!("tunnel connection failed: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}
```

The tunnel protocol is an extension of `TerminalStream`: loopflow.studio wraps client streams and routes them to the lfd tunnel based on session_id lookup.

## Done when

```bash
# Local test (no relay)
# 1. Start lfd
lfd

# 2. Create wave and trigger interactive step
lf wave create test --flow design
lf wave run test

# 3. Connect via gRPC (simulates mobile)
grpcurl -d '{"session_id": "..."}' localhost:50051 loopflow.control.v1.ControlService/TerminalStream
# Stream shows terminal output, typing sends input
```

Observable outcomes:
- `ConnectWave` returns `session_id` field
- `TerminalStream` RPC available in proto
- Typing in stream appears in PTY
- PTY output appears in stream
- Reconnecting with `replay_from_seq` shows buffered output
- Terminal resize changes PTY dimensions

Relay testing (requires loopflow.studio changes) is out of scope for this phase but the lfd implementation should work unchanged.

## Open questions

1. **Buffer size tuning**: 100KB is a guess. Should we make it configurable? Profile memory impact with many sessions?

2. **Heartbeat/keepalive**: Should we add explicit heartbeat messages, or rely on HTTP/2 PING frames? Mobile networks drop idle connections.

3. **Session timeout**: If mobile disconnects and doesn't reconnect within X minutes, should we kill the agent? Or let it run indefinitely?

4. **Multiple clients**: Can two mobiles connect to the same session? Current design says no (single subscriber). Is that right?
