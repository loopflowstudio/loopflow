# Assumptions

- The first serial PR ends at the durable local-first transcript and repeated
  failure roll-up. Inline typed references remain a follow-up because they need
  navigation and preview contracts beyond the chat store boundary.
- The existing durable POST response is the send acknowledgement for this
  slice: the server returns immediately after journaling and remains independent
  of agent startup and trace capture. Offline queuing is explicitly excluded.
