# 01: Metering Infra

Parse token data from all three harnesses and capture prompt composition. Rust only — no UI, no new HTTP endpoints.

## Status

- **Shipped:** 2026-02-26
- **Validation run:** `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all`, `uv run pytest python/tests/`, `swift test --package-path swift`, `tests/e2e/test_smoke.sh`, `uv run pytest tests/e2e/test_api_smoke.py -v`
- **Notes:** Concerto UI tests were locally sensitive to signing/runner bootstrap setup (`CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` required for CI-aligned invocation).

## What to build

Two new `SessionEvent` variants (`TurnUsage`, `ContextSnapshot`) that flow through the existing event stream and persist to `session_events` without schema migration.

## Data structures

```rust
// In sessions/types.rs

/// Token usage for a single agent turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
}

/// Prompt composition snapshot at session start.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshot {
    /// Tokens per source category ("step", "direction", "diff", "area", "repo_doc", etc.)
    pub sources: HashMap<String, u64>,
    /// Total context budget available
    pub budget: u64,
    /// Total tokens used
    pub total: u64,
    /// Diff representation tier ("UnifiedDiff", "StatOnly", "None")
    pub diff_tier: String,
}
```

Add to `SessionEvent`:

```rust
pub enum SessionEvent {
    // ... existing variants ...

    /// Token usage for a completed turn. Emitted after TurnCompleted.
    TurnUsage {
        turn_id: String,
        usage: TurnUsage,
    },

    /// Prompt composition snapshot. Emitted once at session start, before first TurnStarted.
    ContextSnapshot {
        snapshot: ContextSnapshot,
    },
}
```

## Key changes

### Claude harness — `claude_mapping.rs`

In the `"result"` arm (line ~207), extract token data from the event JSON before emitting `TurnCompleted`:

```rust
"result" => {
    flush_text_delta_parser(events, state, turn_id);

    // Extract usage before emitting completion
    let usage = TurnUsage {
        input_tokens: event.pointer("/usage/input_tokens")
            .and_then(Value::as_u64).unwrap_or(0),
        output_tokens: event.pointer("/usage/output_tokens")
            .and_then(Value::as_u64).unwrap_or(0),
        reasoning_tokens: event.pointer("/usage/reasoning_tokens")
            .and_then(Value::as_u64),
        cache_read_tokens: event.pointer("/usage/cache_read_input_tokens")
            .and_then(Value::as_u64),
        cache_write_tokens: event.pointer("/usage/cache_creation_input_tokens")
            .and_then(Value::as_u64),
        model: event.get("model").and_then(Value::as_str).map(String::from),
        cost_usd: event.get("cost_usd").or_else(|| event.get("total_cost_usd"))
            .and_then(Value::as_f64),
    };

    let status = if event.get("is_error")...;

    let _ = events.send(SessionEvent::TurnCompleted { turn_id: turn_id.to_string(), status });
    let _ = events.send(SessionEvent::TurnUsage { turn_id: turn_id.to_string(), usage });
    return true;
}
```

### Codex harness — `codex.rs`

In the `"turn/completed"` arm (line ~300), extract usage from `params`:

```rust
"turn/completed" => {
    // ... existing turn_id resolution and tag_parser logic ...

    let status = codex_mapping::map_turn_status(&params);
    let _ = event_tx.send(SessionEvent::TurnCompleted { turn_id: tid.clone(), status });

    // Extract usage from params
    let usage = TurnUsage {
        input_tokens: params.pointer("/usage/input_tokens")
            .and_then(Value::as_u64).unwrap_or(0),
        output_tokens: params.pointer("/usage/output_tokens")
            .and_then(Value::as_u64).unwrap_or(0),
        reasoning_tokens: params.pointer("/usage/reasoning_tokens")
            .and_then(Value::as_u64),
        cache_read_tokens: None,
        cache_write_tokens: None,
        model: params.get("model").and_then(Value::as_str).map(String::from),
        cost_usd: None,
    };
    let _ = event_tx.send(SessionEvent::TurnUsage { turn_id: tid, usage });
}
```

### OpenCode harness — `opencode_mapping.rs`

In `complete_turn` (line ~123), accept optional usage data. The caller in `map_status` extracts it from `properties`:

```rust
fn complete_turn(
    state: &mut ReaderState,
    status: TurnStatus,
    usage: Option<TurnUsage>,
    mapped: &mut MappedEvent,
) {
    let turn_id = state.current_turn_id.take().unwrap_or_else(|| "unknown".to_string());
    state.tools.clear();
    mapped.events.push(SessionEvent::TurnCompleted { turn_id: turn_id.clone(), status });
    if let Some(usage) = usage {
        mapped.events.push(SessionEvent::TurnUsage { turn_id, usage });
    }
}
```

Extract from `properties` in `map_status`:
```rust
let usage = properties.get("usage").map(|u| TurnUsage {
    input_tokens: u.pointer("/input_tokens").and_then(Value::as_u64).unwrap_or(0),
    output_tokens: u.pointer("/output_tokens").and_then(Value::as_u64).unwrap_or(0),
    cost_usd: u.get("cost").and_then(Value::as_f64),
    ..Default::default()
});
complete_turn(state, TurnStatus::Completed, usage, mapped);
```

### ContextSnapshot emission — `sessions/mod.rs`

Change `prepare_session_prompt` to return the breakdown alongside the configs.

In `create_session` (line ~133), after `prepare_session_prompt` and before `spawn_harness_startup`, emit the snapshot:

```rust
let (session_config, agent_config, breakdown) = self.prepare_session_prompt(...)?;

// Emit context snapshot before harness starts
let snapshot = ContextSnapshot::from(&breakdown);
self.append_runtime_event(
    &session_id, &runtime,
    SessionEvent::ContextSnapshot { snapshot },
).await?;
```

The `From<&ContextBreakdown>` conversion:

```rust
impl From<&ContextBreakdown> for ContextSnapshot {
    fn from(b: &ContextBreakdown) -> Self {
        Self {
            sources: b.source_tokens.iter()
                .map(|(k, v)| (k.as_str().to_string(), *v as u64))
                .collect(),
            budget: DEFAULT_CONTEXT_BUDGET as u64,
            total: b.total() as u64,
            diff_tier: format!("{:?}", b.diff_tier),
        }
    }
}
```

### Event bridge — no changes needed

The catch-all arm in `spawn_harness_event_bridge` (mod.rs line ~505) already forwards any `SessionEvent` variant to `append_runtime_event`. New variants flow through automatically.

### Event type string — `types.rs`

Add to `event_type()`:

```rust
Self::TurnUsage { .. } => "turn_usage",
Self::ContextSnapshot { .. } => "context_snapshot",
```

## Constraints

- All new fields on `TurnUsage` that providers may not report are `Option`. Never panic on missing data.
- `ContextSnapshot` uses `String` keys (not `DocumentSource`) so the serialized form is stable across refactors.
- No schema migrations. Events serialize as JSON into the existing `session_events.data` column.
- `TurnUsage` must derive `Default` for the OpenCode `..Default::default()` pattern.

## Validation

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
```

Write tests for:
- Claude result event parsing produces correct `TurnUsage` (extend existing `claude_mapping` tests)
- Codex turn/completed parsing produces `TurnUsage`
- OpenCode status transition produces `TurnUsage`
- `ContextBreakdown` → `ContextSnapshot` conversion preserves source tokens
- `TurnUsage` and `ContextSnapshot` round-trip through serde JSON

## Done when

After running a session through lfd, `session_events` contains `turn_usage` entries with non-zero `input_tokens` and `output_tokens`, and a `context_snapshot` entry with source token counts. Verified by:

```bash
cargo test --all
# Then run a session and inspect events:
# curl localhost:4400/sessions/{id}/events shows turn_usage and context_snapshot events
```
