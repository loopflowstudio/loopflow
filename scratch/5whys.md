# Why "every provider" Tasks still left Antigravity behind

## Timeline (from `git log`)

| when | commit | what it did to idle wake |
|---|---|---|
| 11-19 | `2a3b6d11d8` #1985 | added the row `as an idle work session, hears a steer`: Claude yes, Codex yes, OpenCode and Pi `no: next turn only (Task 540073)` |
| 11-20 | `a51ae5e48f` #1998 LOO-1013 | added the Antigravity column by copying its neighbour's cell: `no: next turn only (Task 540073)`. Task 540073's brief names only OpenCode and Pi |
| 11-23 04:14 | `1fe6570713` #2008 LOO-1022 | made one behavior (installed skills) a required `Capabilities` field and one test over `HarnessKind::ALL` |
| 11-23 06:10 | `3578f78f6d` #1988 LOO-1002 = Task 540073 | flipped OpenCode and Pi to yes; Antigravity's cell kept pointing at the Task now closing |
| 11-23 17:41 | `43e031870e` #2032 LOO-1068 | titled "every provider"; its README text says "on any of the four" and "Antigravity takes queued steers on its next turn" |

LOO-1068 wrote the gap down. Nobody owned it.

## Five whys

1. **Why does an idle `agy` work session not wake?** `idle_conversation_candidates`
   (`rust/loopflow-storage/src/sqlite/steers.rs`) only returns rows with a Codex
   endpoint, a Claude inbox key or an OpenCode/Pi socket. `agy` writes none, so
   the SQL filter drops it. Leaving a harness out costs nothing: no error, no
   log, no failing test.
2. **Why did no test notice?** `server::wake::tests::` is written per reach
   variant (inbox, app-server, socket), not per `HarnessKind`. A harness without
   a variant has nothing to fail. The only tests over `HarnessKind::ALL` are
   refusals, transient retry and installed skills (`harness/conformance_tests.rs`).
3. **Why was the gap "owned" by a finished Task?** Gap ownership is a table
   cell in `harness/README.md`. The Antigravity column was filled in by copying
   the cell next to it. Nothing checks that the named Task covers that harness,
   and when the Task closes its cells don't change.
4. **Why did "every provider" and "a new harness can't skip it" not generalize?**
   Each Task made its own behavior structural (LOO-1022: one required field;
   LOO-1068: one shared boundary function) and counted the harnesses it knew
   about. Nobody had to list every behavior for every harness, so scope came
   from whoever wrote the brief.
5. **Why was `agy` left for later in the first place?** It has no provider
   inbox. Probed on 1.5.16 (2026-11-24): `agy agentapi send-message` needs the
   running server's address and CSRF token, which agy hands only to tool
   commands and sidecars. Hooks get `ANTIGRAVITY_CONVERSATION_ID` alone, MCP
   servers get nothing, sidecars load only from `~/.gemini`, and agy writes no
   discovery file. The hard part was put off, and the README was the only
   place that said so.

## What closes this kind of miss

- One typed table of every behavior a person notices, per `HarnessKind`, with
  each answer being "supported" or a gap with a reason and owner. Adding a
  harness or a behavior fails to compile until each cell is filled in.
- A behavior's test loops over `HarnessKind::ALL` and checks what the table
  says, both ways: a supported cell must work, and a gap must still be a gap.
  So closing a gap means updating the table.
- The README table is generated from that table and checked by a test, so
  the docs can't drift or keep a dead Task reference.
- Idle wake is chosen per harness from `Capabilities`, not inferred from
  whichever store columns happen to be filled. A harness can't leave it out
  silently.

What this doesn't solve: a gap's owner Task closing while the gap stays open.
The table needs an owner but can't see Task state offline. `lf` could check
that at gate later.
