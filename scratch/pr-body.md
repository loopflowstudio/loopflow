Switching a live ordinary Session from Claude to Codex preserves its identity and history and accepts continuation input. Previously, stopping the old attachment retired the Session, so the same switch rejected its own snapshot with `invalid control authority: conversation changed before input replacement`. A recorded departure now leaves the Session open; natural exits and stale-input checks keep their existing behavior.

Bare Sessions use the directory fallback instead of `default`. Standard OSC title updates run through the output owner and VT parser, between complete provider sequences. They recover from stale provider titles and replay on reattachment. Name reads stay off the I/O loop; cmux/tmux host controls and manual workspace names remain intact.

The recorded scope was “can we try to make sure rex/herdr/cmux/tmux all well supported, but also generalize the default”. This change covers the provider-departure repair, default naming and generalized title publication. It preserves current main's typed AgentProcess IDs, directory reporting and asynchronous exit settlement.

## Validation

- 322 affected Rust checks pass across the initial and targeted repair passes: lifecycle/concurrency, provider switching, naming/host policy, VT parsing, transport, startup cost and CLI contracts. The 127-check repair pass passed in full.
- Fixed the obsolete single-OSC assertion, made provider-fixture output atomic, and made shutdown fixtures wait for socket cleanup as well as the completion receipt. The repeated shell test completes all 20 lifecycles without leftover sockets or owned jobs.
- Current stable Rust 1.99.1: formatting, test inventory and `cargo clippy --all-targets -- -D warnings` pass.
- Architecture checks, 760 headless Swift tests and Swift boundary checks, and 73 website tests pass. Website verification uses the candidate CLI.

## Host evidence and limits

Disposable checks on the pre-reconciliation branch build observed the initial title, `Meaningful rename`, and restoration after stale provider OSC 2 in Rex, tmux and Herdr 0.9.3. Rex's title API reported the values; tmux's window and pane followed them; Herdr's `terminal_title` followed them while its manual tab label remained `1`. Rex display preferences and Herdr layouts remain unchanged.

Live cmux remains unverified: the app launched and ping briefly returned `PONG`, then its socket vanished before fixture creation. Host-policy checks pass; this is not a live cmux pass.

Linux Clippy/transport and the disposable installation boundary remain with CI. No build was installed during this gate. The linker emitted the previously observed macOS unwind-size warning; compilation succeeded. Full recovery evidence is archived separately from the PR; unrelated upgrade investigations and the already-landed config migration repair are excluded.
