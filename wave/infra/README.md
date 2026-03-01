# Infrastructure Hardening

## Vision

Close the gaps between "works on my machine" and "production-grade." The codebase is architecturally sound — clean subsystem boundaries, real test pyramid, consistent patterns. But long-running daemons leak resources, CI is slower than it needs to be, and several high-value code paths have zero test coverage.

Not a rewrite. Not new features. Tighten what exists.

## Strategy

Work from the inside out: data integrity first (migrations, resource leaks), then CI feedback speed, then test coverage for the paths that matter most.

Each sprint is a standalone PR that makes the system measurably more reliable. No sprint depends on another — pick them in any order based on what hurts most.

The review identified issues across all 7 subsystems (lfd daemon, engine, Concerto, CLI/ops, Python client, deploy/CI, testing). Sprints are organized by impact, not by subsystem — a single sprint may touch multiple areas.

## Goals

- Long-running lfd daemons don't accumulate unbounded resources
- CI gives feedback in minutes, not 15+
- Every background trigger loop has at least one test
- No silent data corruption path in migrations
- Security defaults are safe (no open webhook endpoint)

## Risks

- Adding CI caching introduces cache invalidation bugs (stale artifacts)
- Test coverage sprints could produce tests that test implementation rather than behavior

## Metrics

- CI wall-clock time (target: <5min for rust-test job)
- `lfd` uptime without restart (target: weeks without resource issues)
- Test count for trigger loops (target: >0)
