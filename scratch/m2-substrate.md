# M2 Substrate Design Notes

M2 should land as one coherent PR that removes the old center: postgres,
container mode, and the dual-backend persistence shape. The destination from
`wave/goals/architecture-direction.md` is narrow sqlite as local operational
scratchpad, chat/journals for conversation, git/GitHub for PR truth, Asana for
roadmap truth, and explicit `lf` commands at authority boundaries.

## M1 Review

M1 landed the component-boundary part of the architecture, but not the full
public grammar described in the M1 scratch doc.

What looks aligned:

- `conversation` is now a neutral top-level component rather than
  `lfd::conversations`.
- `harness` is a top-level component, with OpenCode runtime moved out of
  `lfd`.
- `engine::repo`, `engine::wave_config`, and `wave::subscription` reduced the
  worst command/daemon import cycles.
- `crate::wave` no longer appears to import the old command/http/executor
  implementation details that the M1 cycle list called out.

What needs M2/M1-follow-up attention:

- User-facing dispatch is still `lf q worker run`. The code still has
  `QCommand`, `WorkerCommand`, `Placement::Fresh`, `Placement::Pool`, and
  `Placement::Stack`, and docs/prompts still teach `lf q worker run`. The M1
  design direction was ordinary `lf` invocations plus placement flags:
  `--dispatch`, `--stack`, and `--fork`.
- The current worker API still launches detached tmux sessions. Jack's stated
  direction was that placement flags should run the agent in the placed
  worktree and block like a normal `lf` command.
- The fresh top-level module is named `conversation`. Jack now wants the product
  word to be `chat`. M2 should decide whether to rename `conversation` to
  `chat` before deeper substrate work makes the old word stick.
- The `lfd::conversations` shim deletion is now clean in HEAD, including the
  harness import/log-label cleanup. `cargo check -p loopflow` passed after that
  cleanup.

Review refs:

- `rust/loopflow/src/lf/commands/q.rs:1` still documents and implements
  `lf q worker run`.
- `rust/loopflow/src/lf/mod.rs:242` still defines `QCommand` /
  `WorkerCommand`.
- `rust/loopflow/src/wave/mind.rs:294` still teaches the resident to dispatch
  with `lf q worker run`.
- `rust/loopflow/src/engine/launch.rs:308` still asserts that generated prompts
  contain `lf q worker run`.
- Current uncommitted M1 cleanup also deletes the one-line
  `lfd/http/routes/wave_config.rs` shim and calls `engine::wave_config`
  directly from `routes/mod.rs`.

## M2 Target

Delete the remote-center substrate, then leave the remaining store boring and
local.

The PR should:

1. Remove postgres support completely.
2. Remove container/compose mode as a product and service deployment shape.
3. Simplify `lfdb` to a sqlite-only local registry.
4. Narrow sqlite's job to machine operational facts.
5. Move conversation/chat authority to the chat tier, not the registry.
6. Introduce or clarify the query plane so readers do not reach through random
   daemon/store internals.
7. Keep useful deployment hardening only where it still applies to native/SSH
   operation.

## Current Substrate Surface

Primary deletion areas:

- `rust/loopflow/src/lfdb/postgres.rs`
- `rust/loopflow/src/lfdb/rows.rs` if it only exists to abstract rusqlite vs
  tokio-postgres rows.
- `StorageConfig::Postgres`, `StoreBackend::Postgres`, `StoreError::Postgres`,
  and `StoreError::PostgresPool` in `rust/loopflow/src/lfdb/mod.rs`.
- Postgres branches in `rust/loopflow/src/lfdb/migrations.rs`.
- `SqlDialect::Postgres`, `postgres_override`, and postgres catalog rendering
  in `rust/loopflow/src/lfdb/catalog.rs`.
- `Mode::Container`, `RuntimeBackend::Compose`, `StorageType::Postgres`, and
  docker executor mode coupling in `rust/loopflow/src/lfd/config.rs`.
- `rust/loopflow/src/lfd/service/compose.rs`.
- Compose branches in `rust/loopflow/src/lfd/service/{macos,linux}.rs`.
- `LFD_DATABASE_URL` handling in `rust/loopflow/src/lfd/mod.rs`,
  `rust/loopflow/src/bin/lfd.rs`, service env allowlists, and tests.
- HTTP error mapping for postgres-specific uniqueness handling in
  `rust/loopflow/src/lfd/http/routes/repos.rs` and generic HTTP store error
  mapping in `rust/loopflow/src/lfd/http/mod.rs`.
- `tokio-postgres` and `deadpool-postgres` dependencies once the code compiles
  without them.

Keep sqlite mechanics that are still real:

- Store location and migration for `~/.lf/lfd.db`.
- Run/session lifecycle rows needed by local `lf` invocations and lfd's
  read/push bridge.
- Provider token storage if the local daemon still refreshes provider auth.
- Attention/live PR facts that are local cache or coordination affordance,
  not authority.
- Repo registry/edges if local commands still need a durable machine index.

Questionable sqlite tables/API after narrowing:

- `chat_memory_blocks` and `chat_messages`: conversation history should live in
  chat journals. If these are now dead compatibility organs, delete their store
  traits, migrations after the current schema point if unused, and callers.
- `queue_blocks` / `queue_merge_events`: if merge queue truth now lives in git,
  GitHub, and `lf op`, keep only derived cache that is actively read.
- `fork_runs`: if M1/M2 placement flags replace old fork-run machinery, delete
  or move this into the execution placement component.
- summary rows: decide whether summary is chat-derived memory, query cache, or
  still wave operational state.

## Proposed Implementation Shape

First, settle vocabulary:

- Rename `crate::conversation` to `crate::chat` if "chat" is canonical.
- Rename public types only when they cross the user/product boundary. Internal
  wire/event names can remain temporarily if renaming them would dominate the
  substrate PR.
- Update docs/prompts to say "chat" for the conversation tier.

Then remove postgres:

- Make `StorageConfig` a sqlite path wrapper or remove it entirely if callers
  can pass `PathBuf`.
- Make `Store` hold `sqlite::SqliteStore` directly. Delete `StoreBackend` and
  all dual-backend match arms.
- Delete `lfdb/postgres.rs`.
- Collapse catalog rendering to sqlite-only, or inline SQL in `sqlite.rs` if
  the catalog no longer earns its indirection.
- Remove postgres migration functions and tests. Keep existing sqlite
  migrations; do not squash applied sqlite history unless Jack explicitly wants
  a local-db reset story.
- Remove postgres-specific error variants and map sqlite constraint failures
  through domain errors rather than database-type checks where possible.
- Remove `tokio-postgres` conversions on `LfdId` if no remaining code needs
  them.

Then remove container mode:

- Delete `Mode::Container`, `RuntimeBackend::Compose`, and
  `StorageType::Postgres`.
- Make `mode` either absent/native-only or remove `mode` from config entirely
  if no alternative remains.
- Delete compose generation and docker compose service management.
- Simplify macOS/Linux service install/start/status/uninstall to native only.
- Remove `LFD_DATABASE_URL` from service env allowlists and config resolution.
- Keep `LFD_AUTH_TOKEN`, webhook limits, trusted proxy CIDRs, output retention,
  and local path validation if still used by native/SSH operation.
- Re-home any still-useful credential env/mount validation only if there is a
  native executor path that consumes it. Do not keep docker-shaped config just
  because it was already written.

Then narrow sqlite/query:

- Define the "query plane" as the read API used by `lf`, `lfd`, `wave`, and
  future `lfq`. It should compose authoritative sources rather than making
  sqlite pretend to own everything.
- Reads over operational rows can stay in `lfdb`.
- Reads over chat history should go through `chat` journal readers.
- Reads over PR/branch truth should go through git/GitHub-facing ops helpers.
- Aggregation should be derived/disposable.

## Design Questions

1. Should M2 begin by renaming `conversation` to `chat`? The word matters more
   now than later because M2 will decide which tier owns history and summaries.

2. Is the unfinished placement grammar part of M2's scope, or should it be a
   separate M1 follow-up before M2 starts? The substrate work can proceed
   without it, but the registry narrowing gets cleaner if `lf q worker run`,
   `Placement::Pool`, and fork-run leftovers are already gone.

3. When removing old config, should M2 hard-error on `mode: container`,
   `storage: postgres`, and `LFD_DATABASE_URL` with explicit messages, or
   silently ignore removed keys/env? The codebase style points toward hard
   errors for config that used to mean real behavior.

4. Should `mode` survive as `native`-only, or disappear from config now that
   there is no product mode choice?

5. Which sqlite domains remain authoritative enough to keep?
   Candidate keepers: run/session lifecycle, provider tokens, repo registry,
   attention/live PR cache. Candidate deletions/moves: chat messages, chat
   memory blocks, queue merge events, fork runs, summaries.

6. What should the query plane be called and where should it live? Options:
   `engine::query` for shared composition, `lfdb::query` for sqlite-only reads,
   or a top-level `query` module that composes sqlite + chat + git/GitHub.

7. Do we preserve any container-mode hardening in M2, or explicitly defer it to
   M3's SSH-first self-hosted story? The useful pieces are env allowlisting,
   named credential mounts, service-file secret hygiene, and health checks.

8. Does M2 update user docs/prompts away from `lf q worker run`, or only remove
   postgres/container docs? If placement is out-of-scope, docs will remain
   split-brained.

## Done When

- `rg -n "postgres|Postgres|LFD_DATABASE_URL|deadpool_postgres|tokio_postgres" rust/loopflow/src rust/loopflow/tests` returns no product-code hits except unrelated prompt examples or historical notes that deliberately remain.
- `rg -n "mode: container|Mode::Container|RuntimeBackend::Compose|docker compose|compose::" rust/loopflow/src rust/loopflow/tests docs README.md` returns no live product/docs hits.
- `rg -n "rusqlite::" rust/loopflow/src/wave rust/loopflow/src/engine rust/loopflow/src/harness` returns no hits; sqlite stays in `lfdb` or the explicit query substrate.
- `cargo fmt` passes.
- `cargo test -p loopflow` passes, or the doc records any intentionally deferred long/integration checks.
- README/docs no longer teach removed postgres/container behavior.
- If `conversation` is renamed, docs/prompts consistently say `chat`.
