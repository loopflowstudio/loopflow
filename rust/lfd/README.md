# lfd (Rust) Local Dev

```bash
# Postgres + lfd (Docker)
docker compose -f rust/lfd/docker-compose.yml up --build

# Stop
docker compose -f rust/lfd/docker-compose.yml down
```

```bash
# Override HTTP port
LFD_HTTP_PORT=8081 docker compose -f rust/lfd/docker-compose.yml up --build

# External Postgres port override
LFD_PG_PORT=5433 docker compose -f rust/lfd/docker-compose.yml up --build
```

```bash
# One-time init (new volume or missing DB)
docker compose -f rust/lfd/docker-compose.yml up -d postgres
docker compose -f rust/lfd/docker-compose.yml exec -T postgres \
  psql -U postgres -c "CREATE DATABASE lfd;"
docker compose -f rust/lfd/docker-compose.yml run --rm lfd migrate
docker compose -f rust/lfd/docker-compose.yml up -d lfd
```

```bash
# Point at an external Postgres instance
LFD_STORAGE=postgres \
LFD_DATABASE_URL=postgres://postgres:test@localhost:5432/lfd \
cargo run -p lfd
```

## API Endpoints

```bash
# Health check
curl http://localhost:2486/health

# List waves
curl http://localhost:2486/api/waves

# WebSocket (live events)
websocat ws://localhost:2486/ws
```
