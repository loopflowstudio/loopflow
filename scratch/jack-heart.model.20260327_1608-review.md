# Branch validation: wave crons + concurrent ingest coordination

## Try it

Create a wave config with supplemental crons and inspect the returned wave JSON/UI payload for `crons`, then run concurrent ingest tests:

```bash
cargo test ops::ingest::tests::concurrent_ingest_picks_different_items
```

## Needs CI confirmation

- Concerto UI tests: `ConcertoUITests-Runner` exits before bootstrapping (`signal kill`) locally. Swift package tests pass.
