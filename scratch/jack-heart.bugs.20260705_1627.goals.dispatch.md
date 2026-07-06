# lfq: run any `lf` command on the wave server

## Problem

A sandboxed mind can't `lf --dispatch`. Dispatch does two writes that land
outside the mind's sandbox:

1. `create_run_worktree` (`lfd/executor/helpers.rs:175`) shells `git worktree
   add` against the **main repo's `.git`** — a sibling of the mind's cwd
   (`resident.rs:68` puts the mind in `<repo>.<wave>`, not the main checkout).
2. The dispatched flow runs **inline** in the same `lf` process
   (`bin/lf.rs:380`, `run_target_in_repo`) and must write the *new* run
   worktree — another sibling.

Any workspace-scoped sandbox (Codex `workspace-write`, Claude Code's bash
sandbox) makes only the mind's cwd writable and read-only-protects `.git`.
Both writes are outside that root → denied. That's the "current .git write
restriction." (A stock loopflow codex resident runs `danger-full-access` and
wouldn't hit this — the goals mind is a hand-launched Codex CLI under the
default `workspace-write`.)

## Shape

Do **not** change `lf --dispatch`. The remote path is explicit through a new
thin binary, `lfq`:

```bash
lfq run implement "add endpoint" --wave goals --dispatch
lfq run op commit -m "wip" -p
lfq run review --wave goals
```

`lfq run <lf args...>` forwards the argv verbatim to the wave server, which
runs `lf <args...>` **unsandboxed** as its own child, streams stdout/stderr
back, and exits with the child's code. `--dispatch`'s meaning is unchanged;
only *where it executes* moves — so both writes above happen server-side where
nothing is sandboxed.

`run` is a real subcommand, not the whole grammar: the top-level `lfq`
namespace stays free for native verbs later (`lfq attach <run-id>`, `lfq ls`,
`lfq logs`). Those are out of scope for v1.

**Explicit means explicit.** Unlike `lf chat`, `lfq` does **not** fall back to
local when no server is listening — it errors. `lf` is the local door; `lfq`
is the remote one; no silent blurring between them.

## Pieces

### 1. `lfq` binary — `rust/loopflow/src/bin/lfq.rs`

A pure transport. No business logic, no `lf` grammar awareness — it forwards
`argv` opaquely.

- Parse only its own frame: `lfq run <argv...>`. Everything after `run` is the
  untouched `lf` command line.
- Resolve the wave endpoint with the machinery `lf chat` already uses:
  registry (`wave::registry::wave_server_endpoint`) → `.wave-endpoint` pointer
  (`engine::wave_context::read_endpoint_pointer`). The wave comes from
  `--wave` in the forwarded args, else the ambient wave (`AmbientWaveRef`,
  same as chat).
- Auth: read the resident token (`server::read_resident_token(main_repo,
  wave)`, or `RESIDENT_TOKEN_ENV`) and send `RESIDENT_TOKEN_HEADER`. The mind
  can read the main repo, so the token file is reachable.
- Send `cwd` so the server runs from the caller's directory (same machine).
- Stream the response to stdout/stderr; exit with the propagated code.
- No server listening → error with the chat-style hint ("is `lf wave`
  running?"). No local fallback.

Register the bin in `Cargo.toml`. Keep it tiny — the weight is server-side.

### 2. Server door — `POST /exec` on the wave server

`wave/server.rs:337` router, gated by `state.resident.authorize(&headers)`
like `/messages` and `/memory`.

Request DTO (wire type — no defaults, per CLAUDE.md):

```rust
struct ExecRequest {
    argv: Vec<String>,   // the lf command line, sans the `lf` argv[0]
    cwd: String,
    wave: String,
}
```

Handler:
- Spawn the **`lf` binary specifically** (never a shell): resolve `lf` next to
  the running server exe (mirror `path_for_children()` in `mind.rs:220` so it's
  the same binary), `args(argv)`, `current_dir(cwd)`, inherit the server's
  unsandboxed environment.
- Stream stdout+stderr back as a chunked body; trailer or final frame carries
  the exit code. Axum streaming `Body`.
- The child inherits nothing of a sandbox because the server itself has none.

### 3. LOOPFLOW.md guidance (lands **with** the code, not before)

Append to the Delegate Work section — default to `lf`, fall back to `lfq run`
on a sandbox wall:

> Dispatch and other git-mutating ops (`--dispatch`, `lf op commit/pr/land`)
> write the main repo's `.git`, which a sandboxed harness can't touch. If a
> command fails on a `.git` write restriction, run the **same command** through
> `lfq run` — it executes on the wave server, unsandboxed:
> ```bash
> lfq run implement "<task>" --wave <wave> --dispatch
> ```
> `lfq run <cmd>` is `lf <cmd>` executed remotely; the local `lf` semantics are
> identical.

## Open details

- **cwd policy — DECIDED: client cwd.** The server runs `lf` from the caller's
  `cwd` (same machine, so the path is valid server-side). Faithful to what a
  local `lf` would have done from that directory.
- **Streaming granularity.** Line-buffered is enough for v1; interactive steps
  (a flow that prompts) won't work over one-shot exec — that's what the future
  `lfq attach` is for. Note the limit; don't solve it now.
- **Long-running `--dispatch`.** Today `run_placed_target` runs inline, so
  `lfq run … --dispatch` blocks for the whole flow (mirroring local `lf`). If
  dispatch later detaches into its own session, `lfq run` inherits that for
  free. Out of scope here.

## Security

`/exec` is a localhost RCE door by construction. Constraints:
- Bind localhost only (the wave endpoint already is `127.0.0.1:<port>`).
- Require the resident token; reject unauthenticated calls like the other
  resident doors.
- Exec the `lf` binary directly with an argv vector — never `sh -c`, no shell
  interpolation of the forwarded args.

## Demo

From a sandboxed Codex session in the goals wave worktree:

```bash
lf implement "…" --wave goals --dispatch     # fails: .git write restriction
lfq run implement "…" --wave goals --dispatch # succeeds: runs on the server
```

The dispatched run appears in `lf runs` / the wave, PR and all.
