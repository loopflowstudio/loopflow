# Relay status payload

Older Desktop clients still consume the provider transition on reconnect.
Keep the relay payload and the Session DTO intact. The Desktop socket test
uses the shared fork fixture to exercise a status reading with history.
Pane tests cover the reported fresh and resumed headers.

The quiet opening spinner uses “Opening session” as its VoiceOver label.
