# Steering design questions

Task `lf-9c03797`, 2026-10-11. Jack Heart accepted pursuit; these decisions remain
open. The [design](session-steer-subscriptions.md) owns scope, reviewed defaults,
provider evidence and acceptance requirements.

- **Handoff authority:** choose proven provider-originated emission or explicit
  fenced adapter rebinding. Neither has Jack Heart's approval. Codex preserves its
  provider/hook declaration across attachment changes; the launch claim goes stale.
  Reading a replacement claim alone cannot authorize an old callback. Production
  cutover awaits this decision and its handoff/replacement counterexamples.
- **OpenCode boundary identity:** 1.18.33 supplies no request ID to its messages
  transform and retries the transformed input internally. Specify boundary/retry
  identity and pin a protocol fixture without treating a local callback ID as a
  provider request or an ephemeral transform as durable confirmation.
