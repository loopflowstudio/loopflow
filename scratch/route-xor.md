path: done

Ship-ready. The branch delivers the full `remote-lfd-connection` intent end to
end: `lf op pair` mints a 90-day ledger token and prints a terminal QR + URL,
iOS gains the single setup screen (Scan QR / Paste link / Sign in with
Loopflow), `loopflow://pair` deep links route into the existing connection
stack, and WS close 4401 surfaces a re-pair state instead of a spinner.

Validation re-run on the current tree (code unchanged since the gate commit
0532cbc3 — the two later commits are scratch/route bookkeeping only):

- `cargo fmt --check` — pass
- `cargo clippy -p loopflow -- -D warnings` — pass
- `env -u LF_RUN_ID cargo test -p loopflow pair --lib` — pass (11 tests)
- `scripts/test_pairing_smoke.py` — pass (`pair_url_shape`,
  `paired_token_http`, `paired_token_websocket`)
- `swift test --filter PairingPayload` — pass (5 tests)
- `check_swift_multiplatform_boundaries.py` — pass

The full Rust + Swift + iOS-build suite was exercised at the gate commit per
`scratch/jack-heart.mobile.20260518_1805-review.md`; nothing in code changed
after, so re-running the long iOS build adds no signal. PR copy is complete
and accurate; `.pr-copy-ref` refreshed to current HEAD so `lf op land`
consumes the cached body. Scope held to the view-only charter — no
write/build/land/chat surfaces. No reason to iterate.
