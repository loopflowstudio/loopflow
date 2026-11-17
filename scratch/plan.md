# CI repair

Remaining: run the subscription frame test with the revision-value assertion, format and lint, then publish and arm PR #1960 through `lf`.

Failed head: `534cc5c396c091287e1730d04335f005b54fc3e2`.
CI job 134263754150: `subscribe::a_frame_is_its_answer_and_body` expected four fields; the emitted frame also contained `read_at`. The other 557 tests passed.

The first local test passed after another process updated the field assertion during compilation. A second run verifies the added revision-value assertion.
