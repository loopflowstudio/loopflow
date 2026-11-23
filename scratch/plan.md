# LOO-1002: wake idle Antigravity work sessions

## Outcome

`lf steer LOO-12 'use the new API'`, or a Task update at a step transition, reaches an
open, idle work session at once on every harness: a turn starts in that terminal with lf's
message. Live case: MTG-415's Task conversation runs on `agy` and misses the Task updates sent
at step transitions while it sits idle.

OpenCode and Pi shipped on main in #1988 (LOO-974): plugin/extension socket under
`/tmp/lf-wake-*`, `{"thread","text"}` line, `started` answer, OpenCode turn reporting and
reopen wake. This branch merged it and kept its protocol; its own earlier plugin socket is gone.
What this Task still delivers is Antigravity (Task comment, 2026-11-23: "Scope includes
Antigravity").

Done when the README row "as an idle work session, hears a steer"
(`rust/loopflow-execution/src/harness/README.md:123`) says `yes` for all five harnesses
(it does on this branch) and a real `agy` work session is woken by `lf steer`.

## Demo

In a Task checkout: `lf task run LOO-x -a antigravity` (`agy` 1.5.14 installed), let the work
session finish its turn and sit idle, then `lf steer LOO-x 'reply with exactly WOKEN'` from
another shell. Within a second the `agy` TUI shows lf's message as a prompt and answers WOKEN,
with nobody typing. `lf session steers <id> --json` shows the steer delivered. Needs the branch
installed: the Machine server runs the wake watcher.

## Built (on this branch, tests passing after the merge)

- `antigravity.rs` `prepare_wake`: a work session's launch makes a random token, sets
  `ANTIGRAVITY_SERVER_CSRF_TOKEN` in `agy`'s environment, passes `--log-file`, writes the wake
  file `{log, token}` (0600) beside the context delivery in `<LF_HOME>/context-deliveries/`
  (`<uuid>.wake`, `<uuid>.agy.log`), and records it with `SqliteStore::record_launch_inbox`
  (attachment-keyed). Chats and batches get none.
- `LanguageServer::find` reads the port from the log at each wake (the last line ending
  `for HTTP`; `agy` logs its gRPC port with the same prefix), `send` posts
  `SendUserCascadeMessage` with `x-codeium-csrf-token` and `cascadeId` = the provider thread.
- `IdleReach::Antigravity(wake file)`; `idle_conversation_candidates` picks it when the row has
  no endpoint or socket and `provider IS 'agy'` (the store's name, `HarnessKind::as_str`).
- `wake.rs` `Reach::Antigravity`; test
  `an_idle_antigravity_conversation_takes_its_news_from_agys_language_server` (TcpListener
  standing in for the language server).
- Docs: server README reach list, harness README row and Antigravity paragraph, `docs/sessions.md`.

Probe facts behind it (`agy` 1.5.14): the launch env token becomes the language server's token;
the POST started a turn in an idle TUI, rendered as a prompt, `PreInvocation` and `Stop` fired;
hooks never see the token. The RPC is undocumented: a release that breaks it leaves a wake
failure in `logs/lf.log` and the steer arrives at the next boundary. Sessions launched by the
released `lf` have no wake file and stay next-turn-only.

Checks: `cargo test -p loopflow --lib server::wake::tests::` 9 passed;
`cargo test -p loopflow-execution --lib harness::antigravity::tests::` 8 passed, 1 ignored
(both 2026-11-22, after merging #1988).

## Remaining

1. **Live wake** (demo/qa, needs the branch installed and an `agy` login): the demo above. Risk
   still open: a wake racing a typed prompt on `agy` is untested; the server sends only when the
   store says yielded.
2. **Shape, for review:** the candidate query distinguishes Claude's key from Antigravity's wake
   file by `provider IS 'agy'` inside the `(None, None)` arm. Main's `Inbox` enum already names
   what provider code listens on; `Inbox::Antigravity` (or a column-free check on the file) could
   replace the extra `IdleReach` variant. Small; decide at gate.
3. Main's `tests/e2e/idle_wake.py` drives OpenCode and Pi live against a stand-in model; `agy` has
   no stand-in model route, so Antigravity is not added there.
