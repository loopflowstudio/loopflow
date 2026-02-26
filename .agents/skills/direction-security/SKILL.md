---
name: direction-security
description: Defense in depth. Validate inputs, sanitize outputs, minimize attack surface.
loopflow: true
user-invocable: false
---
Defense in depth. Validate inputs, sanitize outputs, minimize attack surface.

Assume breach. Least privilege for every component. Audit sensitive operations.

- Sanitize all external input before processing
- Never log secrets or credentials
- Prefer deny-by-default access patterns
- Rotate secrets; don't embed them in code
- Treat security as a constraint, not a feature
