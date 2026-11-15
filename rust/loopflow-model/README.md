# Shared records

```bash
cargo test -p loopflow-model --lib
```

Conversation events, usage receipts and Program Status records have one owner
here. The application composes these records with provider, store and terminal
effects. These records embed no source-checkout path or Git provenance.

Existing application module paths re-export the same types for integration
fixtures. Public enums retain their non-exhaustive contract across crates.
