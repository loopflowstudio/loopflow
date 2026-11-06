# LOO-717 — simulated design review

Jack Heart requested a simulated Mitchell Hashimoto review on November 3, 2026.
This was an agent-authored application of public writing, not Mitchell's review,
quotations or endorsement. Full assessment and source links are preserved at
`a1067c99cc:scratch/loo-717-design-review.md`.

The lasting recommendations are incorporated in
[the accepted contract](loo-717-shell-lifetimes.md): Desktop and Rex consume the
same Session retention policy while owning native presentation; disappearance
needs explicit recovery semantics; two-minute agent ending and fifteen-minute
checkout cleanup use independent conditions; cleanup honors Jack's uncommitted
file disposal policy; automatic post-deletion restoration stays outside scope.

Review still needs the working native terminal experience: same execution and
state through detach/reattach, Close settling owned jobs, remote failures without
local fallback, Session-only checkout retention, and actual process/PTY/socket
cleanup. Headless checks do not establish native or physical remote-Machine proof.
The demo record and current design own remaining work. The original review granted
no implementation approval; Jack's subsequent approval is recorded in the design.
