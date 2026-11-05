# Task 4877de: assumptions and open decisions

Parent (LOO-593) decisions and retry history: `67a968bf84:scratch/questions.md`.

## Decided in kickoff — November 5, 2026 (headless; Jack Heart can overturn)

- **Cancel of an in-flight read answers nothing.** The Task's stated default.
  The client forgets the id; a late response for an unknown id is logged at
  debug and dropped.
- **No second control connection for bulk reads.** Measure first; the
  revision-2 column in `server-performance/results.md` is the evidence.
- **One error shape.** Request failures and connection-level failures are both
  `error {id, code, message}` with `id` null before a request exists. The
  revision-1 `result: {kind: error}` is deleted rather than kept beside it.
- **`instance` moves to `status` and `welcome`.** Installation replacement
  reads it from `status`; per-message `instance` had no other consumer.
- **`lf server status` prints JSON without a flag today.** The demo reads
  `revision` from that output; no `--json` flag is added.
- **Line codec buffers before rejecting.** `LinesCodec` with a 32 MiB maximum
  buffers up to the limit before erroring, where the length prefix rejected
  before allocating. Accepted for a same-user local socket; the limit holds.
- **The LOO-718 handoff lives in this plan**, not in `mobile-backend.md`. That
  file is the parent's; editing it here would conflict with the parent's land.
  The handoff text is the plan's "Phone (LOO-718) adopts" section.

## Scratch hygiene

Parent scratch notes were removed from this checkout (see
[loo-593-parent.md](loo-593-parent.md)). If LOO-593 lands before this Task,
`server-protocol-2.md` and `server-performance/` disappear from main; re-add
`results.md` and the revision-2 samples from this branch's history.

## Decided during implementation — November 5, 2026 (headless)

- **`lf server status` gains a `listening` boolean** beside `revision`, `instance`
  and `error`. Installation's replace decision reads it (a server accepted the
  connection, whatever it answered) in place of the deleted per-message
  `instance.is_some()` test. Still no `--json` flag; the output was already JSON.
- **`Read::read` returns `serde_json::Value`, not a string.** `show_json`,
  `status_json`, `saved_layout` and `codebase` return values too (compress
  removed their text round trip; CLI callers print the value). `config
  repository --path` is a JSON string. `served_reads` compares `Value`s.
- **The subscription owns a blocking `Runtime`** (the shared watch `Reader`), so
  the server drops the `Subscription` through `spawn_blocking` when a watch ends;
  dropping it on the async task panics ("cannot drop a runtime …"). The bail path
  and normal end both route through one `watch` wrapper that does this.
- **A revision-1 refusal keeps the revision-1 `{id,instance,result}` envelope**
  so an older `client::status()` still reads `instance`; the new probe reads a
  closed connection (its `{"ty` prefix looks like a 1.9 GB length to an old
  server) and reports "older protocol".

## Deferred to demo/review — November 5, 2026 (headless)

- **The Swift `ServerPerformanceTests` measurement first failed here** because
  `session resume --prepare` needs a harness selection; realign (below) named
  `-a claude` and the measurement passed. Demo/review owns the rendered run.

## Decided during compress — November 5, 2026 (headless)

- **Subscriptions leave through a drop guard, not an awaited `spawn_blocking`.**
  Aborting the subscription task (cancel, connection end) dropped the shared
  watch's blocking runtime on a tokio worker; tokio swallowed the panic
  ("Cannot drop a runtime in a context where blocking is not allowed") and the
  cancel test passed while printing it. `Watch` in `service.rs` moves the drop to
  a blocking thread however the task ends. The underlying coupling (the watch
  `Reader` owns a `Runtime`) is LOO-593's `watch.rs` and is left as is.
- **A client-side EOF is "older protocol" only before `welcome`/`status`.**
  `Socket::receive` returns `None` on a clean close; mid-session EOF reads
  "closed the connection; the outcome is unknown".
- **Check line:** `cargo test -p loopflow --test served_reads` — 4/5 under
  load, `server_operations_match_their_lf_commands` passed alone; realign's
  rerun below was 5/5.

## Decided during realign — November 5, 2026 (headless)

- **The performance fixture names `-a claude`.** The deferred measurement failed
  only because `lf session resume --prepare` resolves a harness it never starts
  and this host has several installed with no saved preference; the command
  prints "--agent has no effect" yet fails without it. Naming Claude follows the
  repository's agent policy. The command's own inconsistency is not this Task's.
- **Two revision-2 runs are recorded,** loaded and quiet host, rather than the
  better one alone; `results.md` reads the difference to baseline against the
  spread between them.
- **Check line:** `cargo test -p loopflow --test served_reads` — 5/5 passed in
  68 s with all five concurrent; the compress-step effect-comparison failure did
  not recur. `swift test --skip-build … --filter ServerPerformanceTests` passed
  twice, `workload_connections` 1.

## Decided during concept review — November 5, 2026 (headless)

- **A null-id `error` is the connection's last message.** The README's "about
  the connection" rule, applied uniformly: the server closes after `invalid`
  for a malformed post-hello line or a repeated `hello`/`status`, as it already
  does for `busy`, `incompatible` and pre-hello refusals. Swift already treats
  it as fatal; the Rust client already ends its wait. Applied the same day.
- **`ErrorCode::Unsupported` is deleted.** Nothing emits it; an unsupported
  mutation reports through the command's exit code and stderr. Applied the
  same day.
- **Desktop's double decode (`ServerClient.read` re-encoding `value` to text
  for `RegistryQuery`) is a later Task**, not this slice: the string path is
  shared by subprocess and served reads across every Desktop reader.
- **No `scratch/concept-review.html` page.** The headless review records its
  findings in the working design; the page is for a review with a person.
- **Second pass (same day): the client bounds its handshake.** Revision 1's
  `client::connect` gave every wait ten seconds; revision 2's
  `Connection::start`/`attach` and Swift's `ServerLink.open` wait without
  limit, and Desktop serializes every read behind one opening task. Implement
  restores the ten-second handshake bound on both sides and clears it for the
  session and the bridged stream. Not a product choice; no wire change.

## Decided during implement (handshake bound) — November 5, 2026 (headless)

- **The bound is one constant per side**, `HANDSHAKE` in `client.rs` (shared by
  `status()`, `start` and `attach`) and `ServerLink.handshakeSeconds`; the
  stalled-server tests wait the real ten seconds rather than taking a test-only
  timeout parameter.
- **A stalled wait names what it waited for** ("did not welcome the connection
  / open the owner bridge / answer the readiness request within 10s") and points
  at `lf server status`; other read errors keep their own text.
- **Check line:** `cargo test -p loopflow --lib server::` 11/11 (10 s);
  `swift test --filter ServerClientTests` 6/6 (20 s); `cargo test -p loopflow
  --test transport_tests server_` 15/15; `cargo fmt --check` and `cargo clippy
  --all-targets -- -D warnings` clean.

## Decided during compress (handshake bound) — November 5, 2026 (headless)

- **`Connection::start` leaves the handshake bound set.** `open_at` clears it
  once welcomed; `attach` keeps it through the `bridged` answer and clears it on
  the raw stream, instead of clearing and re-arming between the two. Desktop's
  three copies of "Server disconnected; refresh state before retrying a command"
  are one `ServerLink.disconnected`. No wire or behavior change.
- **Check line:** `cargo test -p loopflow --lib server::` 11/11 (10 s);
  `swift test --skip-build --filter ServerClientTests` 6/6 (20 s); `cargo fmt
  --check` and `cargo clippy --all-targets -- -D warnings` clean. served_reads,
  transport_tests and the measurement were not rerun: the handshake path they
  exercise did not change behavior, and gate reruns them once.
