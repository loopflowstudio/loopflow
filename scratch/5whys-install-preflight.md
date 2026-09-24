# 5 Whys: published candidate cannot inspect itself

## The Problem

The laptop's published-release install fails before promotion can obtain valid
candidate JSON. Restoration is pending, not completed; this analysis follows a
safe reproduction of the reported failure. Full design and acceptance live in
`make-laptop-refresh-and-lf-installed-schedule-proof.md`.

## Chain

Verified download → child preflight emits nothing → ordinary startup refuses
retained bytes → read-only inspection shares the runtime gate → tests cover
each component without crossing the real startup boundary.

**Why 1:** The promoting process parses empty child stdout as JSON, producing
EOF. The child had already emitted the inactive-artifact error on stderr.
Both steps reproduced with published v0.12.19, in `install-incident-20260924.json`.
Earlier detection: require real candidate JSON before any promotion side effect.

**Why 2:** The downloaded candidate's standard preflight is treated as ordinary
CLI execution. `Preflight` is absent from the installer startup exceptions in
both release and Task source, although `Promote` and `LocalPreflight` are present.
Earlier detection: execute the nested `promote --preview` → preflight path with
retained candidate bytes and an active development selection.

**Why 3:** Ordinary startup intentionally rejects inactive retained bytes by
hash. The downloaded CLI equals the published fallback while development is
active. The wrong assumption is that candidate inspection needs active runtime
authority. Removing LF context does not alter this result. The runtime rule
itself is correct and has an explicit copied-artifact regression.

**Why 4:** CLI startup policy and the read-only install dispatcher sit at
different points in `bin/lf.rs`. Standard preflight reaches authorization before
its documented early read-only dispatch, so that documented contract is false
in the retained case. Keep the existing command boundary consistent; do not
create another authority or broadly bypass all install commands.

**Why 5 (root supported by current evidence):** Verification does not compose
the relevant boundaries. `tests/local_promotion.rs` calls `build_preview`
directly. `python/tests/test_shell_installer.py` substitutes a successful shell
script for the candidate. `machine_install.rs` tests retained runtime rejection.
All can pass together while the actual preflight executable is unreachable.
The missing proof is candidate inspection under a retained machine selection,
with ordinary execution still refused.

## Unanswered Whys

| Branch point | Unexplored question | Priority |
|---|---|---|
| Why 4 | Which historical change omitted standard preflight, and was that deliberate? Current and published source establish the bug, not author intent. | Low; not needed to choose the repair |
| After restoration | Will published promotion pass migration/executable compatibility and preserve all active Sessions once JSON is reachable? | High; must observe, never assume |
| Publication | When will an independently authorized release contain #1273 and the repair? Current latest remains v0.12.19. | Blocking live acceptance; no release launched here |

## Fixes

| Level | Fix | Prevents |
|---|---|---|
| Immediate | Make standard candidate preflight reach its existing read-only dispatcher; deliver through an authorized published release. | Exact inspection failure; restoration still needs the configured demo |
| Structural | Preserve runtime authorization while exempting candidate inspection at the existing CLI boundary. | Confusing inspection with activation |
| Verification | Run actual candidate/child startup with retained published and active development state in an isolated OS account; retain incompatible-JSON and ordinary-command refusals. | Green component tests hiding a broken composition |

## Changes to Implement

- [ ] Correct standard preflight startup routing, without weakening runtime checks.
- [ ] Add real process-boundary regression and unchanged-state assertions.
- [ ] Preserve actionable child failure diagnostics; never manufacture JSON success.
- [ ] After publication, restore configured install and repeat; inspect Sessions/history.
- [ ] Complete outstanding actual scheduled and explicit catch-up proof on this Task.
