# Transient agent failure recovery

`lf <skill>` should survive temporary provider failures without making the
human restart a shell pipeline or losing the agent's partial-work context.

## Design

- Classify only provider error payloads and stderr for known transient capacity,
  rate-limit, availability, and transport failures. Never retry an arbitrary
  nonzero agent exit.
- Retry headless launches on a bounded 2s, 5s, 15s, 30s backoff ladder.
- Resume Codex and Claude sessions when the failed attempt reported a provider
  session id. Send a continuation prompt instead of replaying the original task.
- Represent subscription exhaustion separately from transient request-level
  rate limits. Record the exhausted managed account's reset/cooldown, then
  immediately select another routed account and start from the current workspace
  with the original task attached. Never override an explicit `LF_ACCOUNT`.
- Keep each retry visible in logs and trace it as another failed/completed turn
  in the same explicit capture. Internal launches that use implicit capture keep
  one capture per attempt.
- Return the final nonzero result after the retry ladder is exhausted so existing
  callers preserve their error behavior.

## Done when

- A scripted capacity failure followed by success retries once, resumes the
  reported session, and returns success.
- Permanent failures run once.
- Retry exhaustion returns the last failure.
- A subscription-limit result is typed, cools the account, and fails over without
  trying to resume the exhausted account's provider session.
- Codex and Claude command builders carry the resume id correctly.
- `cargo fmt`, focused tests, and `cargo clippy -- -D warnings` pass.
