# Branch review: jack-heart.harness.20260216_1432

## What was implemented

- Added persistent chat memory blocks end-to-end:
  - new `chat_memory_blocks` DB migration/table,
  - store trait + SQLite/Postgres implementations,
  - HTTP routes: list/upsert/delete memory blocks per wave,
  - DTO + typed model wiring.
- Added Concerto chat UX for waves:
  - new Chat tab in `WaveDetailPanel`,
  - `ChatState` state machine and memory prompt builder,
  - Anthropic client integration,
  - memory block CRUD UI in `WaveChatView`.
- Added loopflow gate command plumbing:
  - new `.lf/config.yaml` keys `lint` and `test`,
  - new CLI commands `lf ops lint` and `lf ops test` that run configured commands.
- Updated built-in gate/lint/rebase step prompts and testing docs to steer contributors toward repo-configured lint/test commands.
- Added regression coverage for chat/memory behavior, config parsing changes, keyboard timing stability, and memory block position/name normalization logic.
- Fixed a Swift integration bug in `LocalWaveService` memory methods so requests use shared request/session/auth/error handling (`makeRequest` + `performRequest`) instead of an undefined direct `session` path.

## Key choices

- **Memory blocks are wave-scoped and ordered by `position`**: simple ordering model, deterministic rendering, and stable prompt construction.
- **Upsert-by-name semantics**: treating `(wave_id, name)` as identity makes memory editing predictable from UI and API.
- **`lint`/`test` as shell command strings in config**: avoids hardcoding toolchains (Rust/Python/Swift/etc.) and keeps gate behavior repo-specific.
- **Chat context limited to explicit memory blocks**: avoids accidental hidden context and keeps A1 behavior easy to reason about.

## How it fits together

`WaveChatView` drives `ChatState`, which loads/stores memory blocks through `WaveServiceProtocol` (`LocalWaveService`). `LocalWaveService` calls new lfd memory-block endpoints, which persist through `RunStore` implementations into SQLite/Postgres with migration `007_chat_memory_blocks`.

For CLI quality gates, `.lf/config.yaml` now defines canonical lint/test commands; `lf ops lint` / `lf ops test` read and execute those commands from repo root.

## Risks and bottlenecks

- Anthropic chat is network/API-key dependent; missing/invalid keys degrade to explicit in-UI error bubbles.
- Memory block names are user-controlled identifiers; renames are implemented as delete+upsert and depend on successful delete first.
- `lint`/`test` command strings execute via `sh -c`; malformed commands fail at runtime and are only validated when invoked.
- Chat prompt context currently scales linearly with memory content size.

## What's not included

- No tool-calling/agent-loop chat orchestration yet (this is single-shot assistant replies with memory context).
- No advanced memory conflict resolution/versioning beyond last-write-wins upsert.
- No server-side validation beyond non-empty memory block names.
- No automatic migration/backfill of legacy chat memory formats (new table starts fresh).
