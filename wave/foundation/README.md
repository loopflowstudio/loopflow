# Foundation

## Vision

The system works correctly. Tests cover the important paths, code is clean, APIs are complete, deployments are validated. Not new features — making what exists reliable.

### Not here

- Security hardening (that's Trust)
- New features or capabilities
- UI work (unless it's fixing broken behavior)

## Strategy

Map the gaps first (test coverage), then clean up (dead code, layer violations), then validate on real infrastructure (dogfood), then fill API gaps (remote APIs), then harden the container boundary.

Each sprint is a standalone PR that makes the system measurably more solid.

## Goals

- Every background trigger loop has at least one test
- No cross-layer imports, no duplicate utilities, no dead code
- Remote lfd works reliably on native hosts (not just Docker-on-EC2)
- APIs are complete enough that Concerto doesn't need local filesystem assumptions
- Container boundary is documented and intentional

## Risks

- Test coverage sprints could produce tests that test implementation rather than behavior
- Remote dogfooding may surface issues that cascade into other waves
- API expansion scope could creep beyond what remote Concerto needs

## Metrics

- Test count for trigger loops (target: >0 per module)
- CI wall-clock time (target: <5min for rust-test job)
- Remote smoke suite pass rate on Mac Mini (target: parity with EC2)
- Remote API latency for typeahead use cases (target: acceptable for WAN)
