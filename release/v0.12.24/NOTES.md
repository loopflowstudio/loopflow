# v0.12.24

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.24 makes unattended Tasks easier to steer and recover. A Task keeps its chosen agent through later execution, failed decisions expose a blocker, and work with no observed progress can appear as Stalled. The implementation loop also gets shorter, with code compression followed by a focused review and an explicit decision about what comes next.

## Keep the agent chosen for the Task

A Task's agent choice now persists instead of being lost during unattended execution. This lets individual Tasks use a different agent without changing the configured default.

- The Task choice overrides skill defaults through launch, resume, restart, and review.
- Tasks no longer ignore the agent choice supplied with `-m` during unattended execution.

## See when work needs intervention

Failed decisions previously could leave work looking ready. CLI and desktop now share blocked and stalled execution states with recovery guidance, keeping the failed Run available for investigation.

- A failed decision retains its Run and opens a keyed unblock Session.
- Five minutes without events or sampled body/tool CPU progress shows **Stalled**.
- Missing or mismatched samples remain **Unknown**. Observing a stall does not grant authority to terminate the work.

## Reach the next decision with fewer steps

The `pursue` Flow now runs implement → compress → review-slice → loop-decide. Each step has a clearer job: change the code, simplify it, report the evidence and remaining blockers, then choose the next move.

- `compress` edits code; `review-slice` reports replacement evidence and convergence blockers.
- `concept-review` stays interactive.
- Shipping removes the redundant `task-gate` wrapper.

## Operational notes

- Build resource checks gate only on free disk space. Oversized sibling checkouts produce warnings; the current checkout cleans its own oversized disposable build roots.
- Recorded validation includes the full materialized Rust suite, affected Python, Swift, and website tests, the app build, and formatting and static checks. Configured Claude launch/resume, a live five-minute stall, rendered Task states, and live proof of the final decision policy remain unverified. Earlier Codex policy evidence used simulated PM and synthetic feedback; no hosted CI result is claimed.

## Small changes

- Updated the development Ruff dependency from 0.16.3 to 0.16.4.