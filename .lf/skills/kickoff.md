---
extends: builtin
---
## In this repository

**Read the code's history before patching it.** Find the subsystem's earlier
Tasks and PRs (`git log` on its files). If it was built for a replaced model,
or this would be its third patch, plan a redesign instead and say so.

**Check every sibling variant.** A fix framed from one provider, harness,
tracker or host shares code with its siblings (Daytona beside Modal, Asana
beside Linear, every harness). Check each variant for differing assumptions
and cover it or note why it doesn't apply.

**A replacement keeps what the old code covered.** When swapping an engine,
list its languages, formats and cases and keep each one.

**Flag security-relevant work up front.** Changes touching credential
forwarding to Modal or Daytona, telemetry or outbound data, mobile or Tailscale
server auth, or install and release publishing need a **Security and privacy**
note in the plan and a call on whether to run `lf security-review` before
delivery.
