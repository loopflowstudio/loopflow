---
extends: builtin
---
## In this repository

**Suggest `lf security-review` on security-relevant changes.** When the diff
touches credential forwarding to Modal or Daytona, telemetry or outbound data,
mobile or Tailscale server auth, or install and release publishing, call out
what changed under what is likeliest to surprise them and suggest running
`lf security-review` before delivering.
