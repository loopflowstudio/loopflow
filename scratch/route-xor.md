path: code-review

This branch is a pure API/infrastructure change: new `POST /v1/sessions/{id}/input` route handler, `input_supported` field on `SessionDto`, harness capability gating, and an integration test. The iPhone Concerto UI is explicitly out of scope. Nothing observable changed in a user-facing UI — the right path is code-review, not demo.
