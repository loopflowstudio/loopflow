# The `/exec` backdoor

## What this is — and what it is NOT

`/exec` is a **backdoor**, not the general mechanism.

The general rule stays: capabilities are `lf` **run directly** by whoever needs
them — daemonless, no listener in the path (C1). Concerto, scripts, a developer
at a shell: they all just run `lf`. `/exec` does not change that and is not the
path everything routes through.

`/exec` is the narrow hatch for the two callers that genuinely **cannot** run
`lf` in the context where the effect must happen:

1. **A sandboxed subagent inside a wave.** It has no `.git` write (the sandbox
   forbids it). To land/commit/next it asks its **outwave** — the enclosing
   wave listener, which runs unsandboxed — to run the `lf` command on its
   behalf. Client: `lfq` (in-wave).
2. **A remote client across machines.** No local `lf`, no local repo. It reaches
   the machine's lfd over HTTP. Client: `lfq` (remote, M3).

Everything else keeps running `lf` directly. If you can run `lf` where the work
needs to happen, you do not touch `/exec`.

## Discovery — what exists today (2026-07-06)

Grepped the tree before building. Findings:

- **`lfq` does not exist.** No binary, no `commands/q`, no `lfq run`, no crate
  target, nothing in Python/Swift. The memory note ("`lfq run <cmd>` may already
  partly exist") is aspirational — the client is entirely unbuilt.
- **No generic exec endpoint** on lfd or the wave listener. lfd's HTTP surface
  (`lfd/http/mod.rs`) has per-verb hand-routes (`/waves/{id}/next|land|combine|
  stop`, `PATCH`/`DELETE` waves) that do git/worktree/`ops`/tmux **in process**;
  the wave listener (`wave/server.rs`) serves `/health /conversation /events
  /messages /channels /memory /resident/*` — no exec path.
- **The exec machinery already exists** and is reused: `resolve_lf_binary()`
  (`lfd/executor/helpers.rs`) resolves the `lf` binary; `hooks.rs` already execs
  `lf` argv detached via `spawn_lf_execs` for webhook fan-out. `/exec` is the
  same move with argv validation + sync capture.
- **Two capability-token seams already exist** — no new auth primitive needed:
  - lfd: `AuthProvider::Bearer { session_token }` (`lfd/auth.rs`), loopback-gated
    (`local_admin_authorized` checks `source.is_loopback()`), enforced by
    `auth_middleware` on the whole `/v0` nest. A `TODO(M3)` there already
    anticipates the local capability token.
  - wave listener: a per-boot `RESIDENT_TOKEN_HEADER` token
    (`generate_resident_token`, written to `wave/<name>/.resident-token`),
    checked by `ResidentDoor::authorize` on every resident route.
- **clap validation is ready:** `crate::lf::Cli::try_parse_from(argv)` — already
  used in tests — is the "does it compile" gate.

Conclusion: build on `resolve_lf_binary` + the existing token seams; do not
invent `lfq` server state or a new auth mechanism.

## Shipped now — increment 1 (additive, lfd side)

`POST /v0/exec`, token-gated, no hand-routes touched, no Swift/Concerto touched.

- **Engine** `crate::lfd::lf_exec` (host-neutral, state-free so the wave listener
  reuses it verbatim):
  - `validate_lf_argv(argv)` → `Cli::try_parse_from(["lf", ...argv])`; empty argv
    and parse failures return the rendered clap error. **No exec on invalid.**
  - `exec_lf(argv, cwd, env)` → `Command::new(lf).args(argv)` (no shell),
    optional `cwd`/`env`, captures `{ exit_code, stdout, stderr }`.
- **Handler** `routes/exec.rs`: `ExecRequest { argv, cwd }` →
  `ExecResponse { exit_code, stdout, stderr }`. Validate, then exec. A non-zero
  `lf` exit is a *successful* door call reporting a failed run (200 with the
  exit code); a refused argv is a 400.
- **Auth** is automatic by placement under the `/v0` auth nest — the loopback
  Bearer token gates it. Verified end-to-end (see tests).

Response contract: the door returns raw exit code + streams, not a per-verb
shape. Clients interpret. (Whether it should stream long-running output instead
of buffering is an open question — see below.)

Tests (all green): argv-rejects-garbage (unknown flag, empty), valid-argv-execs
(runs `lf op doctor`, captures output), **auth-required** (full-router: no token
→ 401; token + garbage argv → 400, proving the gate precedes the validator).

Verify: `cargo build` clean; `cargo clippy --all-targets -- -D warnings` clean;
`cargo fmt --all` clean; the four `routes::exec` + three `lf_exec` tests pass.

## Increment 2 — the in-wave path (design only, not built)

Host the **same engine** on the wave listener (`wave/server.rs`):

- Add `POST /exec` to the wave server router, `ResidentDoor::authorize`-gated,
  running `exec_lf` in the requested `cwd` (default `runtime.repo_root()`) —
  **unsandboxed**, because the listener process is the outwave. `runtime` already
  exposes `repo_root()` and `name()`, so no new state is needed.
- **Open auth question (real finding, must resolve before building):** the wave
  listener's only token today is the **resident** token, held by the mind. A
  worker **subagent** is a different principal and does not hold it. The in-wave
  path needs a token the subagent legitimately has — options: (a) mint a
  per-subagent capability token when the executor spawns it (alongside the
  `LFD_SESSION_ID` env it already sets) and let the listener accept it; (b) let
  the subagent read the resident token file if the sandbox grants that read.
  This is a security boundary — do not guess. Deciding (a) vs (b) gates the
  build. This is why increment 1 shipped lfd-only.

`lfq` client (also increment 2):

- `lfq <lf-args...>` collects the argv, resolves the door (in-wave: the outwave
  listener via `wave/<name>/.wave-endpoint`; remote: the target machine's lfd),
  POSTs `{ argv, cwd }` with its token, prints stdout/stderr, exits with the
  returned code — a transparent proxy so `lfq op land` feels like `lf op land`.
- **Naming consistency (small open question, non-blocking):** memory calls it
  `lfq run <cmd>`; the endpoint is `/exec`. Keep `lfq run` for muscle memory, or
  rename to `lfq exec` to match the door. Decide when the client lands.

## Security model

- **No shell.** argv goes straight to the binary via `Command::args`. There is
  no shell string, no interpolation, no `sh -c` — so no shell-injection surface
  regardless of argv contents.
- **Parse-gated.** Only argv that parses as a real `lf` command execs; anything
  else is a 400 before a process is spawned. The door cannot be used to run
  arbitrary binaries — only `lf` verbs.
- **Token-gated.** lfd: loopback Bearer (existing `/v0` middleware). Wave
  listener: a token the subagent legitimately holds (the increment-2 finding
  above). The door grants no authority beyond what its token already implies —
  it execs exactly what that principal could run as `lf` itself.
- **Not privilege escalation, by design intent:** the door's whole point for the
  in-wave case is that the outwave runs *unsandboxed* what the subagent cannot.
  That is the escape hatch working as intended; the token is what bounds who may
  use it.

## Out of scope (explicitly)

Deleting the per-verb hand-routes, migrating Concerto to `/exec`, and any Swift
change are **not** part of this. `/exec` is a backdoor added beside the existing
surface; the hand-routes keep serving Concerto unchanged.
