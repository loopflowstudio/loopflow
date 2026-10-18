# LOO-416 operator review — October 17

Resolved: reconnect uses typed attach-only dispatch; explicit history recovery is
separate. `WorkspaceSessionLifecycleTests` covers production buttons, ended/failed
attachments and canceled preparation. Preserve that distinction when sharing UI
state transitions. Original finding, steer `cc08dd6f-e960-4928-ba19-505649273a04`
and runner/locator recovery evidence:
`a3e929a7ef8d614c667086af1e1107d1799c6fb1:scratch/continuation-review.md`.

Use a compatible released CLI for shared Work reads; candidate binaries and schema
drafts belong only in disposable test homes. Current execution and remaining work
are in [the handoff](session-lifecycle-handoff.md), not the historical runner state.
