# Algedonic Signals: Validation

## Demo

```bash
# Full repair chain → escalation (≈4 min with backoff delays)
uv run python scripts/demo-algedonic.py

# Skip cargo build if binary is current
uv run python scripts/demo-algedonic.py --skip-build
```

Expected output:
1. Wave created
2. Run executed (step fails)
3. Repair run 1 dispatched (30s delay, `repair_of` links to failed run)
4. Repair run 2 dispatched (60s delay)
5. Repair run 3 dispatched (120s delay)
6. Repair run 3 fails → algedonic attention item created
7. Attention item visible via `GET /attention`

## Tests

```bash
# Repair chain depth counting, backoff delays
cargo test repair_chain

# LF_HOME env var override
cargo test lf_home

# All Rust tests
cargo test --all
```

## Dev lfd isolation

```bash
# Run dev lfd alongside Concerto lfd
LF_HOME=/tmp/lfd-dev scripts/dev-lfq
```
