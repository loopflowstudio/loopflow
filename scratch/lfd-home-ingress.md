# lfd Home-ingress — W2-224 first PR

## Shape

```
Linear webhook → signature verify → parse event → derive delivery id →
  INSERT OR IGNORE into provider_deliveries (dedup gate) →
    if new: webhook::ingest_event (existing domain ops) → update delivery status → 2xx
    if duplicate: 2xx (idempotent)
```

The existing `webhook.rs` functions (`verify_signature`, `parse_event`,
`ingest_event`) are pure and unit-tested. `lfd` wraps them with the durable
delivery inbox — the one piece the directive adds that the current Slice 1
relay lacks.

## User-visible outcome

`lfd serve` boots a machine-level daemon that:
1. Accepts signed Linear webhook deliveries
2. Persists each delivery to a durable inbox **before** acknowledging (2xx)
3. Routes each delivery to the owning Task Session via existing domain ops
4. Deduplicates by provider delivery id (exactly-once across retries + restart)
5. Records the processing outcome (processed / ignored / no_target)
6. Serves `/health` and `/status` for liveness probes
7. Installs as a launchd (macOS) or systemd user (Linux) service

## End-to-end proof

```
# 1. Start lfd (Linear config from env, store required)
doppler run -- lfd serve

# 2. Send a valid signed Linear webhook (issue edit)
curl -X POST http://127.0.0.1:8080/linear/webhook \
  -H "linear-signature: <hmac>" \
  -d '{"action":"update","type":"Issue","data":{"id":"W2-224",...}}'
# → 200 OK, delivery persisted to provider_deliveries, directive applied

# 3. Send the same webhook again (Linear retry)
# → 200 OK, no duplicate processing (delivery_id dedup)

# 4. Restart lfd, send the same webhook again
# → 200 OK, still no duplicate (durable dedup in SQLite)

# 5. Send an unsigned webhook → 401
# 6. Send a webhook for an issue with no Task Session → 200, status=no_target
# 7. curl /health → {"status":"ok"}
# 8. curl /status → {"waves":N,"deliveries":M}

# Service lifecycle:
lfd install     # render + load launchd plist / systemd unit
lfd status      # report loaded/running
lfd uninstall   # unload + remove
```

Unit proof: `cargo test -p loopflow --lib lfd` covers the inbox dedup, the
outcome mapping, and every error state. Integration proof:
`cargo test -p loopflow --test lfd_tests` boots the real binary, delivers
webhooks, and verifies dedup across a daemon restart.

## Source of truth

- **Delivery inbox**: `provider_deliveries` table (new migration
  `0.11.017`). PK = `(delivery_id, provider)`. Append-mostly; pruning is a
  follow-on.
- **Delivery id derivation** (from parsed `WebhookEvent`):
  - `IssueEdit`: `linear:issue:{issue_id}:{revision}` (revision = `updatedAt`,
    monotonic — aligns with the domain-level guard)
  - `Comment`: `linear:comment:{comment_id}` (globally unique)
  - `Ignored`: `linear:ignored:{webhookTimestamp}:{sha256(body)[:8]}`
- **Domain state**: unchanged. `task_linear_observations`,
  `task_linear_ingested_comments`, `child_commands` — the inbox is the ingress
  gate; the domain tables are the second gate. Both are needed: the inbox
  deduplicates *deliveries*, the domain deduplicates *events*.
- **Wave endpoints**: `wave/<name>/.wave-endpoint` (scanned per `/status`
  request, not cached).

## Affected surfaces

- **New migration**: `0.11.017_provider_deliveries.sql`
- **New store module**: `store/sqlite/provider_deliveries.rs` (sync methods on
  `SqliteStore`) + `store/provider_deliveries.rs` (async wrappers on `Store`)
- **Rewritten**: `lfd/mod.rs` — inbox-backed ingress replaces the relay
- **New**: `lfd/service.rs` — launchd/systemd rendering, `#[cfg(target_os)]`
- **Updated**: `bin/lfd.rs` — `serve` always opens the store; adds
  `install` / `status` / `uninstall` subcommands
- **Updated**: `tests/lfd_tests.rs` — inbox dedup, outcome mapping, service
  render tests
- **Unchanged**: `webhook.rs` (consumed via pure functions, not modified)
- **Unchanged**: `ops/linear_observe.rs` (consumed via `webhook::ingest_event`)

## provider_deliveries schema

```sql
CREATE TABLE provider_deliveries (
    delivery_id   TEXT    NOT NULL,
    provider      TEXT    NOT NULL CHECK (provider IN ('linear', 'github')),
    event_kind    TEXT,           -- "issue_edit" | "comment" | "ignored" | null
    target_kind   TEXT,           -- "task_session" | null
    target_id     TEXT,           -- session id | null
    status        TEXT    NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending','processed','ignored','no_target','error')),
    outcome       TEXT,           -- JSON summary
    received_at   INTEGER NOT NULL,   -- unix ms
    processed_at  INTEGER,
    PRIMARY KEY (delivery_id, provider)
);

CREATE INDEX idx_provider_deliveries_status ON provider_deliveries(status);
CREATE INDEX idx_provider_deliveries_received ON provider_deliveries(received_at);
```

## Store methods

- `record_delivery(delivery_id, provider, event_kind, received_at) -> DeliveryRecordResult`
  — `INSERT OR IGNORE`, returns `New` if inserted or `Duplicate` if already
  present (with the existing row's status).
- `complete_delivery(delivery_id, provider, status, target_kind, target_id, outcome, processed_at)`
  — `UPDATE` the row. Called after `ingest_event` returns.

`DeliveryRecordResult` is `{ inserted: bool, existing_status: Option<String> }`.
On `inserted = false`, the handler checks `existing_status`: if `pending`,
re-process (a crash left it unfinished); otherwise skip (true duplicate).

## lfd serve flow (per Linear webhook)

1. Extract `linear-signature` header → 401 if missing
2. `webhook::verify_signature(secret, body, sig)` → 401 if invalid
3. `webhook::parse_event(body)` → 400 if malformed
4. `webhook::within_replay_window(ts, now)` → 401 if stale
5. Derive `delivery_id` from the parsed `WebhookEvent`
6. `store.record_delivery(...)` — if `Duplicate` and status != `pending`, → 200
7. `webhook::ingest_event(store, event, viewer_id, now)` → `WebhookOutcome`
8. `store.complete_delivery(...)` with the outcome mapped to a status:
   - `Ignored` → `ignored`
   - `NoTarget` → `no_target`
   - `SelfAuthored` → `processed` (classified, intentionally skipped)
   - `Edit { .. }` → `processed` (directive applied or duplicate at domain level)
   - `Comment { .. }` → `processed` (follow-up delivered or duplicate at domain level)
9. → 200 (even for `NoTarget` / `Ignored` / `SelfAuthored` — the delivery was
  received and classified; Linear should not retry)
10. Store error during 7–8 → 500 (Linear retries; the delivery row stays
  `pending` and will re-process on retry)

**What this replaces:** the current `--webhook` flag and the forwarding mode
(forwarding to wave servers). `serve` always opens the store and reads Linear
config from env. The forwarding path and its tests are removed; the durable
inbox is the only ingress.

## Service lifecycle

### macOS (launchd)

`lfd install` renders `~/Library/LaunchAgents/com.loopflow.lfd.plist`:
- `Label = com.loopflow.lfd`
- `ProgramArguments = [lfd_path, "serve"]`
- `RunAtLoad = true`, `KeepAlive = true`, `ThrottleInterval = 10`
- `StandardErrorPath = ~/.lf/logs/lfd.log`
- `EnvironmentVariables` — non-secret only (`LF_HOME`, `LF_DB_PATH`); secrets
  sourced from Doppler via a wrapper or `launchctl setenv`
- File perms `0o600`
- `launchctl unload && load` to activate

### Linux (systemd user)

`lfd install` renders `~/.config/systemd/user/lfd.service`:
- `Type = simple`, `Restart = on-failure`, `RestartSec = 5`
- `ExecStart = lfd_path serve`
- `Environment` — non-secret only
- `systemctl --user daemon-reload && enable --now`

### Secrets boundary

Service files never contain `TOKEN`, `SECRET`, or `KEY` env vars. `lfd install`
prints the Doppler command the user should source before starting. The service
file gets `0o600` regardless (it may contain paths).

## CLI

```
lfd serve [--addr 127.0.0.1:8080] [--repo <root>]
lfd install [--addr 127.0.0.1:8080]
lfd status
lfd uninstall
```

`serve` always opens the store (required for the inbox). Linear config
(`LF_LINEAR_WEBHOOK_SECRET`, `LF_LINEAR_VIEWER_ID`) is optional from env — if
absent, the daemon runs but `/linear/webhook` returns 503.

Non-loopback bind: hard refusal without `LF_LFD_AUTH_TOKEN` (following the old
pattern); soft warning with a token.

## Absent and error states

- No store → `serve` refuses to start
- No Linear secret/viewer → daemon runs, `/linear/webhook` → 503
- Bad signature → 401
- Malformed body → 400
- Replay (stale timestamp) → 401
- No target Task Session → delivery recorded `no_target`, 200
- Store error during processing → delivery stays `pending`, 500 (Linear retries)
- Duplicate delivery, already processed → 200 (idempotent)
- Duplicate delivery, still pending → re-process (crash recovery)
- Cannot bind port → process exits

## Operational boundary

- Local-only default: `127.0.0.1:8080`
- Body limit: 256 KiB for webhook routes (`DefaultBodyLimit`)
- Request timeout: 10s (HTTP client)
- No cached wave state: `/status` rescans endpoint files per request
- Delivery inbox is append-mostly: pruning is a follow-on Task

## Exclusions (follow-on Tasks)

- GitHub webhook ingestion (one event type to prove routing)
- `/exec` backdoor
- OAuth / remote identity
- Subscription registration CLI (`lfd register`)
- Stale/degraded subscription state surfacing
- Best-effort nudges to live Wave servers
- Delivery inbox pruning
- Removing `lf pm webhook serve` (non-breaking; stays as CLI path)
