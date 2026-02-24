# Infra

Internal quality and developer infrastructure. Reduce duplication, improve testability, make the codebase easier for agents and humans to work on autonomously.

## Vision

A developer (human or agent) should be able to: build from clean, start the system, exercise any feature, validate it works, and iterate — without asking for help. Infrastructure work earns its place by unblocking other waves or reducing friction across the board.

## Goals

- Store operations have one implementation, not two copies with different async syntax
- Every HTTP API route has e2e coverage that runs against a real server
- Dev scripts in `scripts/` are the primary test infrastructure; `tests/e2e/` is a thin CI wrapper
- Cold-start to running tests requires no tribal knowledge — it's in the scripts

## Metrics

- Zero copy-paste between SQLite and Postgres store implementations for business logic
- `uv run python scripts/test_<feature>.py` exercises every API route
- New contributors (human or agent) can run the full test suite from a clean checkout with `scripts/dev.py test`
