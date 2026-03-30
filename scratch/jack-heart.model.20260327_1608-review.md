# Branch validation: wave crons + concurrent ingest coordination

## Try it

Create a wave config with supplemental crons and inspect the returned wave JSON/UI payload for `crons`, then run ingest claim tests:

```bash
cargo test ops::ingest::tests::ingest_prefers_pm_claimed_item
cargo test lfd::pm::notion::tests::claim_item
cargo test lfd::pm::linear::tests
cargo test lfd::pm::asana::tests
```

## Needs CI confirmation

- Concerto UI tests: `ConcertoUITests-Runner` exits before bootstrapping (`signal kill`) locally. Swift package tests pass.
