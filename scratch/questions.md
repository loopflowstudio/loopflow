# Relay status payload

The corrected Task scope is Desktop presentation. The private status stream still carries the observed transition and reads it on reconnect in `rust/loopflow-execution/src/transport/connection.rs`. Keeping the field preserves behavior for older Desktop clients. Removing the reconnect lookup would be a separate relay contract change.

The new Desktop reader can ignore the legacy `transition` key. Its remaining fields are sufficient to show the connection message and gate input.
