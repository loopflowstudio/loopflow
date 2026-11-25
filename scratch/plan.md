# LOO-1098: wake Antigravity on reopen, display steers inline in `agy`, and enforce harness parity

History and 5 Whys: [5whys.md](5whys.md). Concept review: [concept-review.html](concept-review.html). Live QA: [qa.html](qa.html).

## Built & verified at `76ff5e7438` (passes 1–2)

- **Antigravity reopen with waiting steers & Task updates:** `agy --conversation <id> -i "Read the waiting steers and Task updates for this Session."` fires `PreInvocation` before anyone types, claims and stamps steers only inside the hook, and omits `-i` when nothing is waiting. Verified live on real `agy 1.5.16` (pass 1 at `c0b28b7dc6`, pass 2 at `4414e34eb5`).
- **Required harness interface & unconditional parity tests:** Removed `harness/parity.rs` and the generated README parity table per Jack Heart's direction. `Harness` requires `recognizes_driver`, `caller_session_env`, `sign_in_advice`, `transcript_home`, and `transcript_reader`; `Launcher` requires `capabilities`, `idle_reach`, `classify_failure`, `open`, and `driver`; `Capabilities` has no `Default` and separates `initial_prompt_on_resume: bool` (Codex empty-thread replay) from `reopen_steers: ReopenSteers::{ServerBoundary, PromptHook}`. All behavior tests iterate `HarnessKind::ALL` with narrow inline `TODO` arms only for pre-existing `LOO-1099` and `Task 48edb9`.
- **Harness-owned idle-wake endpoints:** `sqlite/steers.rs::idle_conversation_candidates` returns raw `IdleCandidate` rows (`INDEXED BY agent_sessions_unended_agent`, 3.63 ms for 200 unfinished rows / 0.035 ms for one session); `loopflow-execution/src/steer.rs` routes both server wake (`idle_conversations`, skipping OS process checks when `idle_reach` is `None`) and hook start checks (`is_idle_reachable`) through `Launcher::idle_reach`. Live QA on real Claude, Codex, and Antigravity passed and confirmed this closes a real store hazard (974 Codex rows in `~/.lf/loopflow.db` had `agent_inbox` set; Codex now reads only `agent_endpoint`).
- **Missing-CLI sandbox alignment:** `task_graph_launch_tests::a_step_that_cannot_launch_is_not_started_again` checks the same four system tool dirs (`/opt/homebrew/{bin,sbin}`, `/usr/local/{bin,sbin}`) hidden by `scripts/test_network.py`.

## Pass 3 design — display incoming steers & Task updates inline in Antigravity

Direction from Jack Heart (2026-11-25, `comment:79dd230d-1028-4f14-acb2-90b07b1f3804`):
- `"is this also ging to make it so that agy *displays* the steers it gets"`
- `"Just go ahead and put this in the task please, lets go back"`

### Experience

When an Antigravity (`agy`) conversation receives steers, Task updates, or Task notices — either on reopen (`lf resume --task <id>`) or during a mid-session turn — the person sees them rendered inline in the TUI as clean, readable markdown (no `<lf:steers>` XML tags, no `Complete source: ".lf/prompts/..."` plumbing, no `&#36;` reference escaping), right above the assistant's response. Startup and compaction reference context (Session/Task identity, file listing, scratch/Wave files, active skill) stays collapsed as an ordinary `systemMessage` with no `user_markdown`, so large background context never floods the terminal.

### Model & changes across layers

Antigravity's `PreInvocation` hook contract (`/Users/jack/.gemini/antigravity-cli/builtin/skills/agy-customizations/docs/hooks.md`) supports multiple entries in `injectSteps` and an optional `"metadata": {"user_markdown": "..."}` on `systemMessage`: the TUI renders `user_markdown` inline instead of a collapsible "System Message" row while the model reads only `systemMessage`.

Today, `emit_block` glues steers and reference context into a single `&str` via `delivery.block_with_prefix`, and `Launcher::hook_reply(event, text)` receives only that string. Split the two concepts cleanly without breaking the 10,000-byte `HOOK_CONTEXT_BYTES` combined budget on single-string harnesses (`Claude`, `Codex`, `OpenCode`, `Pi`):

1. **Human-facing steer rendering (`rust/loopflow-execution/src/steer.rs`):**
   - Extend `ClaimedSteers` with `pub display: String` alongside `pub text: String` (model-facing `<lf:steers>...</lf:steers>` stays unchanged).
   - Add `render_display(steers: &[Steer], remaining: usize) -> String`:
     - Ordinary steer: `**Steer** ({author})\n\n{body}`
     - Task update (`source.starts_with("update:")`): `**Task update**\n\n{body}`
     - Task notice (`source.starts_with("notice:")`): `**Task notice**\n\n{body}`
     - Use raw `steer.text.trim()` (no `render_reference` `$` escaping), cap very long individual bodies cleanly at a UTF-8 char boundary (e.g. 2,000 bytes + `\n\n… *(truncated; inspect with `lf session steers <session>`)*`), join multiple items with `\n\n---\n\n`, and append `\n\n*{remaining} more waiting.*` when `remaining > 0`.
2. **Budget-reserving reference context (`rust/loopflow-execution/src/context_block.rs`):**
   - Replace `block_with_prefix(&self, moment, prefix)` with `block_reserving(&self, moment: ContextMoment, reserved: usize) -> Result<ContextBlock, CoreError>` (keeping `block(&self, moment)` = `block_reserving(moment, 0)`).
   - `render_block` computes `budget = HOOK_CONTEXT_BYTES.saturating_sub(reserved)` and ensures the identity + reference context pointer fits within `budget` (failing with the existing budget error when `text.len() > budget`), then fills active skill, manifest listing, whole files, and excerpts up to `budget`.
   - Update the two callers (`server/boundary.rs` test passes `""` -> call `block`; `context_block.rs` test checks `block_reserving` leaves room for a 3,000-byte steer within `HOOK_CONTEXT_BYTES`).
3. **Structured hook reply (`rust/loopflow-execution/src/harness/mod.rs` & `antigravity.rs`):**
   - Change `Launcher::hook_reply` to:
     ```rust
     fn hook_reply(
         &self,
         event: &str,
         context: Option<&str>,
         steers: Option<&crate::steer::ClaimedSteers>,
     ) -> serde_json::Value
     ```
   - Default implementation (Claude, Codex, OpenCode, Pi): joins `steers.text` and `context` into `hookSpecificOutput.additionalContext` (`format!("{}{context}", steers.text)` when both are `Some`, so total bytes stay `<= HOOK_CONTEXT_BYTES`).
   - `Antigravity::hook_reply`: builds `injectSteps: Vec<Value>`:
     - If `context` is `Some(ctx)`: push `{"systemMessage": {"systemMessage": ctx}}` (no `metadata`).
     - If `steers` is `Some(s)`: push `{"systemMessage": {"systemMessage": s.text, "metadata": {"user_markdown": s.display}}}`.
     - Return `{"injectSteps": steps}`.
4. **Hook emitter (`rust/loopflow/src/lf/commands/context.rs`):**
   - In `emit_block`, keep `steers = news.steers.as_ref().map(|(_, s)| s)` separate from reference context:
     - On `ContextMoment::Start | ContextMoment::Compact`: compute `context = Some(delivery.block_reserving(moment, steers.map_or(0, |s| s.text.len()))?.text)`.
     - On other moments (`Tool | Steer`): if `steers.is_none()`, return `Ok(())`; else `context = None`.
   - Call `harness.hook_reply(event, context.as_deref(), steers)`.
5. **Docs & scratch hygiene:**
   - Update `rust/loopflow-execution/src/harness/README.md` (`hook_reply` table entry and Antigravity steer-display note).
   - Update `scratch/5whys.md`'s closing section so it matches the Pass 2 interface-and-tests enforcement model rather than the deleted parity table.

### First slice & verification

Build steps 1–4 first (small, self-contained, directly testable):
1. Unit tests in `rust/loopflow-execution/src/steer.rs` for `render_display` (ordinary steer with author, Task update, Task notice, multi-item batch + `remaining > 0`, unescaped `$` text, long-body truncation).
2. Unit tests in `rust/loopflow-execution/src/harness/antigravity.rs` (and `context.rs`) asserting exact `hook_reply` / `emit_block` JSON shape:
   - `Start` with no steers: 1 `injectSteps` entry, NO `metadata` key.
   - `Start` with steers: 2 `injectSteps` entries — context entry has NO `metadata`; steers entry has `systemMessage` (model XML) AND `metadata.user_markdown` (clean human markdown, no `<lf:steers>` or reference context).
   - `Tool`/`Steer` with steers: 1 `injectSteps` entry with `metadata.user_markdown`.
   - Default `Launcher::hook_reply`: `hookSpecificOutput.additionalContext` combines steers + context within `HOOK_CONTEXT_BYTES`.
3. Update `rust/loopflow-execution/tests/support/agy_cli.py` to record `displayed` (`metadata.user_markdown` values) alongside `context` in `tui-heard.json`, and assert in `session_launch_contract::a_reopened_antigravity_task_hears_repository_direction_once` that initial start displays nothing inline while reopen with a steer and reopen with a Task update each display their message inline without reference context.
4. Live QA on real `agy 1.5.16`: verify inline display of steers and Task updates in the TUI on both `lf resume --task` reopen and a mid-session turn, confirm reference context stays collapsed, and confirm headless `agy` worker still passes.

## Acceptance at gate

```sh
cargo test -p loopflow --test integration documented_commands::
cargo test -p loopflow-storage --lib query_plans::
cargo test -p loopflow-execution --lib harness::conformance_tests::
cargo test -p loopflow-execution --lib harness::launch_matrix::every_harness_runs_all_four_launch_kinds
cargo test -p loopflow-execution --lib harness::environment_tests::
cargo test -p loopflow-execution --lib harness::antigravity::
cargo test -p loopflow-execution --lib steer::
cargo test -p loopflow-execution --lib context_block::
cargo test -p loopflow --lib lf::commands::context::tests::
cargo test -p loopflow --lib server::wake::tests::
cargo test -p loopflow --lib server::boundary::tests::
cargo test -p loopflow --lib task_graph_launch_tests::
cargo test -p loopflow --test session_launch_contract a_reopened_antigravity_task_hears_repository_direction_once
```

## Outside this work

- Already-open Antigravity idle wake: **LOO-1099** (live PTY probes ruled out `?2004h` paste-mode readiness).
- Viewer handoff (multi-viewer `agy`): **LOO-1097**.
- Headless unanswered questions on Claude, Codex, Antigravity: **Task 48edb9**.
- Claude cross-session steer framing (`scratch/qa.html` pass 2 #1: Claude `task-session` primaries acknowledge `<cross-session-message>` steers and wait for typed confirmation while Codex and Antigravity act immediately): separate prompt/framing outcome outside this diff.
