# Shared records

```bash
cargo test -p loopflow-model --lib
```

Conversation events, usage receipts, Program Status, typed identities, saved
Work ownership records and Task discussion syntax have one owner here. Person
display-name normalization is pure; configuration and tracker fetches stay
in the application. UUID normalization, JSON and SQLite conversions stay with their identity type; native provider IDs keep opaque bytes.
Resolved Machine placement, Task execution state and compiled Graph views remain
in the application. The application composes these records with provider, store
and terminal effects. These records embed no source-checkout path or Git provenance.

Existing application module paths re-export the same types for integration
fixtures. Public enums retain their non-exhaustive contract across crates.
