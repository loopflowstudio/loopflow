# Blocked is a decision result

LOO-298 · Implementation of Jack Heart's 2026-09-30 decision; configured acceptance remains open.

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

## Implemented boundary

The command, its agent-issued authority reader and store wrapper are removed.
`blocked` requires `reason`; navigation still requires `summary`. The Ask key
uses the immutable captured-event sequence, so folding old child passes cannot
change the question's identity. Each later blocked turn gets a fresh captured
event and therefore its own question. Existing failed-decision recovery remains
separate and cannot convert provider failure into a successful blocked result.

The first public recovery proof found that projecting Blocked into the navigation
cursor bypassed the Ask after driver restart. Blocked now remains a selected
output consumed by the driver, while only Advance/Iterate project into navigation.
The second public proof passed with a real Codex engine against a local Responses
fixture: two Asks, public ready/complete, one interrupted/recovered driver, three
deciding turns in one AgentSession/native conversation, and four exact consumed
completions including the initial work step. The terminal transport is stubbed;
this does not prove configured interactive acceptance. Final static/key checks
are recorded in evidence.md.
