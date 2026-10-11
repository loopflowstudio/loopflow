# Steering design questions

Task `lf-9c03797`, 2026-10-11. Jack Heart renewed pursuit after the provider gaps
were reported. These are implementation proof obligations, not pending requests
for product approval. The [design](session-steer-subscriptions.md) owns scope, reviewed defaults,
provider evidence and acceptance requirements.

- **Handoff authority:** implement provider-bound context emission through a private
  launch-owned capability and existing Session lock. Preserve it across attachment
  handoff, revoke it on provider replacement, and prove stale/child callback rejection.
  Reading a replacement attachment claim alone never grants authority.
- **OpenCode boundary identity:** 1.18.33 supplies no request ID to its messages
  transform and retries the transformed input internally. Use an adapter invocation
  ID and Session/publication receipts; pin retry/error behavior without treating
  that local ID as a provider request or an ephemeral transform as confirmation.
