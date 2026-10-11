# Steering design questions

Task `lf-9c03797`, 2026-10-11. Jack Heart renewed pursuit after the provider gaps;
these are proof obligations, not pending product approval. The
[design](session-steer-subscriptions.md) owns their decisions and evidence:

- [Startup exclusion/identity](session-steer-subscriptions.md#startup-fence-counterexample--2026-10-11): resolve the demonstrated Claude lock cycle before cutover.
- [Provider authority](session-steer-subscriptions.md#provider-delivery-authority--2026-10-11): prove capability survival across handoff, revocation on replacement and stale/child rejection.
- [OpenCode boundary](session-steer-subscriptions.md#provider-delivery-authority--2026-10-11): pin transform retry/error behavior using adapter invocation IDs and Session receipts; emission remains unconfirmed.
