# The Story

Loopflow is built by one person and runs on itself all day. Its waves plan its
projects, write most of its code, repair its CI, and cut its releases. That
loop is the product's test bed and its proof: nothing ships to the docs that
isn't survived daily at home.

## How it got here

**A context assembler (late 2025).** The first version did one thing: gather
the right context — repo docs, scratch notes, the clipboard — and hand a
well-formed prompt to a coding agent. Paste an error, run `lf debug -c`, watch
it fix.

**A daemon, an API, a distributed system (early 2026).** Then came persistent
agents, a background daemon, an HTTP control plane, remote execution, a
queue. The machinery grew the way infrastructure grows: plausibly, one
justified piece at a time.

**Picking a side (spring 2026).** The retrenchment. Loopflow decided it is
the orchestration layer above the agent vendors, not another chat client.
Interactive work hands off to Claude Code, Codex, or OpenCode in their own
surfaces; loopflow keeps the goals, the delegation, and the record.

**Waves (summer 2026).** The grind loop was replaced by something closer to a
mind: one durable, journal-backed conversation per wave, assembled from
disposable agent bodies. Progress and chat became the same thread. And in the
same stroke, the distributed system was deleted — the daemon, the remote-exec
door, the HTTP routes, all of it. What remained is one binary, a local store,
and the substrates everyone already runs: Linear, GitHub, SSH.

**Now.** The frontier is fleet-scale: waves with Homes on other machines,
provider accounts that rotate on rate limits, ledgers that can prove — not
estimate — how much of the work happened unattended.

Each deletion made the system more true. The [release
notes](https://github.com/loopflowstudio/loopflow/blob/main/RELEASE_NOTES.md)
carry the full chronology, decision by decision.

## The posture

Loopflow is not chasing distribution. It is one person's daily instrument,
sharpened against real work and documented well enough that you can pick it
up. The bet: a tool honed relentlessly on its maker's own fleet beats a tool
shaped by a sales funnel — and the story of that honing is worth telling
either way.

It is free and open source. If it fits your hand too, even better.

## Who

Jack Heart. Early engineer at Yelp, Asana, and Grail; explored the emergence
of higher-level agents out of collaboration at Softmax. Loopflow combines an
expertise in effective collaboration, a preference for simple APIs, and joy
in the creative process.

[hello@loopflow.studio](mailto:hello@loopflow.studio) ·
[GitHub](https://github.com/loopflowstudio/loopflow)
