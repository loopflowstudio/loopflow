# LOO-612 open questions and assumptions

- Assumed (2026-10-26): decoders are per-connection structs, not free
  `fn(line)`; provider correlation (Claude UUID echo, Codex sequence fence,
  OpenCode snapshot diff) cannot be per-line. Reversible.
- Decided (2026-10-26, revises the plan's "projected from facts"): decoder
  output is one ordered `Vec<Decoded>`; display lifecycle stays
  `Decoded::Display`, emitted by the same decode call as the fact. Display and
  provider turns differ: Claude coalesces steer turns into one runner boundary
  (the driver closes it from `TurnCompleted` facts), Codex display usage is
  baseline-delta while receipts are raw `tokenUsage`, and snapshot completions
  record but never display. One parse, two outputs; no second decision.
- Decided: `FactWriter` freezes origin for `TurnAnswered{request}` lazily when
  no origin was frozen at send — OpenCode terminal input and Pi's extension
  read the current attachment exactly as before. Loopflow-sent requests always
  freeze at send.
- Behavior deltas, small and deliberate: Codex client answers to approvals now
  save the availability reading immediately (was applied, saved on next
  message); Codex headless also selects the native conversation through the
  writer (same thread `record_session_connection` writes); Pi's
  `record_session_agent_session` failure now warns like Claude's instead of
  failing start/callback; Pi stream-death and send-failure finishes now save
  `Yielded(false)` through the same path.
- Preserved, not fixed: Claude display unwraps `{"stream_event":{"event":…}}`
  while activity unwraps `{"type":"stream_event","event":…}` — both kept as-is
  inside `claude_decode.rs`; unifying could double-display text.
- Settled by concept review (2026-10-26): account selection is a launch
  fact, not a decoder fact; slice 4 moves to LOO-611's recorder and
  LOO-612 adds no `AccountSelected` variant.
- Assumed: receipt payloads stay byte-identical; reshaping usage belongs to
  LOO-611 (decision 2). This Task adds no migration.
- Open for Jack Heart: whether LOO-611 or LOO-612 lands first. Plan keeps the
  live `ConversationEvent` stream identical so either order works.
