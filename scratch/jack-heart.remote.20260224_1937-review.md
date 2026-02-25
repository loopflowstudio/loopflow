# Review: Remote wave roadmap restructure

## What was implemented

Renumbered the remote wave roadmap around a dogfood-first strategy. Deleted verbose old phase docs (06-09) and replaced them with focused, compressed wave items (01-06). Added a fork executor cleanup design doc in `scratch/` and open questions in `scratch/questions.md`.

## Key choices

**Dogfood before surface expansion.** Steps 1-2 (EC2 + Mac Mini dogfood) come before auth or API work. The old roadmap had deployment as a single step followed by feature phases. The new order forces operational validation before adding complexity.

**Fork cleanup as a gating phase.** Fork executor drift is now an explicit roadmap item (step 3) between dogfood and studio auth, rather than a backlog item. This prevents shipping auth on top of known inconsistencies.

**Compressed phase docs.** Old phases 07 (studio auth, 248 lines), 08 (API expansion, 220 lines), and 09 (hosted, 63 lines) had implementation sketches (Swift/Rust code, API contracts, deployment diagrams). New phases 04-06 strip these to scope/contract/done-when format. Implementation detail will live in `scratch/` design docs when each phase starts.

**Old phase 06 (remote file access) folded into step 0.** Editor/terminal remote launch was already shipped and is now documented as baseline, not a future phase.

## How it fits together

The roadmap now reads: shipped foundation (01-04 infrastructure + step 0 remote UX) -> operational validation (steps 1-2 dogfood) -> code hygiene (step 3 fork cleanup) -> auth/API expansion (steps 4-6). Each step has a clear "done when" and scope boundary. The `scratch/remote-fork-executor-cleanup.md` design doc is ready for the `implement` step when step 3 begins.

## Risks and bottlenecks

- **Design detail deferred.** The compressed phase docs (04-06) are intentionally light. When implementation starts, design docs in `scratch/` will need to fill the gap. The old verbose docs had useful detail (JWT structure, device flow protocol, K8s architecture) that is now only in git history.
- **Cross-repo coordination.** The new multi-repo section in README.md and the studio auth phase both call out dual-repo ownership. The open question about sequencing strategy is captured in `scratch/questions.md`.

## What's not included

- No code changes. This is a docs-only restructure.
- Old phase content (implementation sketches, code samples) is deleted, not migrated. Recoverable from git history when needed.
- No changes to non-remote wave docs except updating stale `remote/07` and `remote/09` cross-references in `wave/security/`.

## Gate fix applied

Updated `wave/security/06-auth-provider-isolation.md` and `wave/security/README.md` to replace stale `remote/07` references with `remote/04` and `remote/09` with `remote/06`, matching the new numbering.
