# Let harness configuration win wherever Loopflow has no explicit setting

Jack (2026-11-05): lf is a layer on top of the harness, not a second source of
preferences. With nothing set in Loopflow, lf stays silent and the harness decides.

## Inventory: what lf passes to each harness

Needs (stay, reason recorded next to the code):
- Claude: `--append-system-prompt(-file)`, `--settings <hooks>`, `--session-id`/`--resume`,
  stream flags, `--max-turns`, `--json-schema`; `--dangerously-skip-permissions` for
  unattended runs, yolo, or a managed execution boundary; worktree-scope sandbox settings.
- Codex: developer instructions, cwd, hooks + trust IDs, shell environment policy
  (`shell_environment_policy.set`, `allow_login_shell`, `features.shell_snapshot` keep lf's
  tool environment reaching hooks); `workspace-write` + `never` for unattended runs;
  `danger-full-access` + `never` for yolo or a managed boundary.
- OpenCode: instructions, plugin, permissions only for yolo/worktree scope.
- Pi: `--append-system-prompt`, `--extension`; model only when the agent string pins one.

Preferences (unset by default, pass through only when a Loopflow setting is explicit):
- Claude Chrome: `chrome` is `Option<bool>`; unset emits no flag.
- Codex service tier: lf no longer sends `serviceTier`; Codex config decides (reverses #976).
- Codex sandbox for attended native Sessions: lf sends neither `sandbox` nor
  `approvalPolicy` unless yolo or a boundary applies; the person answers approvals.
- Model: already pass-through (bare harness name sends no model).

## Parity: Loopflow settings that mirror a harness preference

Unset sends nothing everywhere. Explicit maps to the harness equivalent; a missing
equivalent is named in the setting description (`lf config catalog`).

| Setting | Claude | Codex | OpenCode | Pi |
|---|---|---|---|---|
| `agent` model (`harness:model`) | `--model` | `thread/start.model` | session `model {providerID,id}` | `--model` |
| `yolo` | `--dangerously-skip-permissions` | `sandbox: danger-full-access`, `approvalPolicy: never` | `permissions: allow` | none; Pi keeps its own tool policy (description says so) |
| `chrome` | `--chrome` / `--no-chrome` | none (description says so) | none | none |
| effort / reasoning | no Loopflow setting; nothing sent | same | same | same |
| service tier | no Loopflow setting; nothing sent (was forced `default`) | n/a | n/a | n/a |
| `isolate` | Loopflow account home, not a harness preference | | | |

Headless floors (needs, not preferences): Claude skips prompts unless its own mode
already bypasses; Codex `workspace-write` + `never`; OpenCode answers permission asks
itself; Pi has no prompt. Managed Task boundaries set `skip_permissions` independently.
Both harnesses decide attended-vs-headless in one place: `claude_options(.., stream)` and
`build_codex_thread_start_params(launch, headless)` (the Codex harness passes `!native`;
it no longer strips keys after the fact).

Tests: `agent::tests::attended_codex_keeps_its_own_policy…`,
`agent::session::tests::{unset_chrome_sends_no_flag…, chrome_choice_reaches_no_harness_but_claude,
bare_harness_sends_no_model…}`, `harness::opencode::tests::bare_opencode_sends_no_model…`,
`session_launch_contract::{unset_chrome_leaves_the_claude_setting_alone…, resume loop (yolo per harness)}`.

## Delete — do not maintain
- Done: `CODEX_DEFAULT_SERVICE_TIER` (one absence assertion remains as the parity guard),
  `--no-chrome` emission for an unset setting, the Codex harness's post-hoc
  `approvalPolicy`/`sandbox` removal.

## Checks
- Compress: `cargo test -p loopflow --lib -- agent:: harness::codex::` 48 ok; `--test session_launch_contract` 27 ok;
  `cargo clippy -p loopflow --all-targets -D warnings` ok. Gate owns `config::`, `settings_launch_tests`,
  `config_tests`, `agent_tests`, `context_launch_tests`, `cli_discovery`.
