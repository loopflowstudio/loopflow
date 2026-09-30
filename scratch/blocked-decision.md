# Blocked is a decision result

LOO-298 · Implementation of Jack Heart's 2026-09-30 decision, pending.

The selected successful native completion returns one of:

- `{"decision":"advance","summary":"evidence"}`
- `{"decision":"iterate","summary":"direction"}`
- `{"decision":"blocked","reason":"the question to resolve"}`

The driver opens the existing keyed Ask after the provider turn ends. The key
includes the captured event, so retrying the same result reuses its answer while
a later blocked turn at the same node can ask a new question. Answering the Ask
consumes the exact selected completion and records feedback without moving the
node or pass. Reserving the next input reuses the AgentSession and native
conversation. No command inside the deciding turn reports blocked.

Remove `lf flow blocked`, its agent-issued authority reader, and its store
wrapper. Keep the separate failed-decision recovery path until the current
scope explicitly replaces it; a failed provider turn cannot become a successful
blocked decision. Ordinary Ask and review completion remain independent of
Flow navigation.

Prove required nonempty reasons, keyed recovery, same-conversation follow-up,
unchanged node/pass before a new verdict, and stale-worker rejection. Existing
schema correction must still distinguish invalid output from a valid blocked
result. Do not claim configured acceptance from scripted provider evidence.
