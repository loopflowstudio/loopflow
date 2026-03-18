# 01: Daemon-Aware CLI Contract

**Finish line:** `lf` can run normally in a plain shell, but when it detects an `lfd`-managed environment it emits structured lifecycle events that let `lfd` track runs, sessions, waits, and outcomes without scraping terminal output.

## Context

The runtime reframe only works if `lfd` can observe the real CLI without becoming the place where flow semantics live. Terminal scraping is too brittle: aliases, wrappers, subshells, shell noise, and prompt formatting all make it hard to tell what actually happened. The clean boundary is for `lf` to know what it is doing and report that back in structured form.

This contract has to work for both automated runs that `lfd` starts itself and interactive runs started by a human or agent inside an attached daemon-owned shell. It also needs to survive local-first adoption without painting remote or SSH-style access into a corner.

## What to build

1. **Detection contract.** Define how `lf` detects `lfd`: env vars, auth token, socket/HTTP target, session ID, run ID, wave ID, repo/worktree identity. Keep the contract explicit and versioned.

2. **Lifecycle events.** Define structured events for:
   - command start
   - resolved step / flow
   - wave / run / session correlation
   - interactive wait points
   - completion
   - failure
   - cancellation

3. **Delivery semantics.** Make event delivery reliable enough that `lfd` can reconcile process state and store state without callback shell hacks. If delivery can fail, define retry and fallback behavior deliberately.

4. **Backward-safe CLI behavior.** Outside an `lfd`-managed environment, `lf` should behave like the normal CLI. The daemon-aware path is additive, not a forked CLI.

5. **Parity tests.** Add tests proving that the same `lf <flow-or-step>` command can run with and without `lfd`, with the daemon-aware path adding observability rather than changing execution semantics.

## Open questions

- Should event delivery go over HTTP, a unix socket, stdio side channel, or some combination?
- How much identity should `lfd` pre-assign versus letting `lf` create and report?
- What is the minimum event set that keeps store reconciliation reliable without over-coupling `lf` and `lfd`?

## Done when

- `lf` can detect an `lfd`-managed environment and authenticate back to it
- `lfd` receives structured lifecycle events for both automated and interactive execution
- Run/session attribution no longer depends on terminal scraping or ad hoc shell callbacks
- Running `lf` outside `lfd` still behaves like the normal standalone CLI
- Tests pin the event contract tightly enough that `lf` and `lfd` cannot drift silently
