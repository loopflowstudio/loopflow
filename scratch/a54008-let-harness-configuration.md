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
bare_harness_sends_no_model…, yolo_sends_pi_nothing}`, `harness::opencode::tests::bare_opencode_sends_no_model…`,
`session_launch_contract::{unset_chrome_leaves_the_claude_setting_alone…, resume loop (yolo for
Claude, Codex, OpenCode; the Codex fixture's own `on-request` shows through when lf sends nothing)}`.

Realign (2026-11-05): code matched the plan; two bounded mismatches fixed. `docs/security.md`
claimed headless OpenCode receives `permission: allow`; lf sends permissions only for yolo or
worktree scope and otherwise answers each `permission.asked` once (`harness/opencode.rs`), and
only Claude warns when raising a floor. Pi `yolo` parity had no test; `yolo_sends_pi_nothing` adds it.

## Concept review (2026-11-05)

Reviewed the accepted direction against the branch. One concept carries the change:
a Loopflow setting is a per-launch selector over a harness preference, never a
default for it. Settled consequences, all already in the code:
- Two setting shapes, on purpose. `chrome: Option<bool>` because Claude has an
  explicit opposite (`--no-chrome`) worth selecting per launch; `yolo: bool` because
  it is Loopflow's one-way bypass and `false` means "no opinion" (more-permissive
  vendor config is never downgraded, so there is nothing to send for `false`).
- "Attended" is one product axis with one signal per harness: Claude's `stream`
  (`-p` is always lf-driven), Codex's `ApprovalPolicy::NativeClient`, OpenCode's
  `interactive`. Each is the harness's own fact, not a second flag on `AgentConfig`;
  no merge needed.
- Reversing #976 (forced Codex `default` tier) follows from the direction, which names
  service tier as a preference. #976 guarded copied account config switching Sessions
  to Fast; that guard is gone by decision, not by accident (see `scratch/questions.md`).
- No new mirror settings (effort, tier). The direction removes Loopflow opinions; adding
  per-launch control of effort or tier is a separate Task if wanted.
No open product choice; nothing left half-built.

## Delete — do not maintain
- Done: `CODEX_DEFAULT_SERVICE_TIER` (one absence assertion remains as the parity guard),
  `--no-chrome` emission for an unset setting, the Codex harness's post-hoc
  `approvalPolicy`/`sandbox` removal.

## Checks
- Realign: `cargo test -p loopflow --lib -- agent::session::tests::{yolo_sends_pi_nothing,chrome_choice,bare_harness}` 3 ok.
- Compress: `cargo test -p loopflow --lib -- agent:: harness::codex::` 48 ok; `--test session_launch_contract` 27 ok;
  `cargo clippy -p loopflow --all-targets -D warnings` ok. Gate owns `config::`, `settings_launch_tests`,
  `config_tests`, `agent_tests`, `context_launch_tests`, `cli_discovery`.

## Demo (2026-11-05, headless, standing in for Jack)

Built branch `target/debug/lf`, private root `~/.lf-demo/a54008` (`LF_HOME=…/machine`,
stand-in `claude`/`pi` on PATH recording argv; no live owners, nothing to tear down;
delete the root when no longer wanted). Observed preference flags per launch:

| Launch | Claude | Pi |
|---|---|---|
| unset, `-i` | none | none |
| `--chrome on` / `off` | `--chrome` / `--no-chrome` | none |
| `--yolo` | `--dangerously-skip-permissions` | none |
| pinned model | `--model sonnet` | `--model anthropic/claude-sonnet-4` |

`lf config catalog` names the gaps: `yolo` "Pi keeps its own tool policy"; `chrome`
"Codex, OpenCode and Pi have no equivalent". Codex and OpenCode need protocol stand-ins,
so their half of the table is proven by `--test session_launch_contract` (27 ok, incl. the
Codex fixture's own `on-request` showing through) and `--lib agent:: harness::codex::
harness::opencode::` (59 ok). Matches the parity table; decision: advance.

## Gate (2026-11-05)
- `scripts/test.py --reuse-passing --rust-filter 'test(config::) | test(agent::) | test(harness::{codex,claude,opencode}::) | test(session_record::) | test(lf::commands::{run,replay}::) | test({config,settings_launch,context_launch}_tests::) | test(cli_discovery::) | binary(session_launch_contract) | binary(agent_tests)'`: architecture, rustfmt, inventory, website pass; Rust 312/314, the 2 failures are machine state (below); `cargo clippy --all-targets -- -D warnings` ok.
- `agent_tests::{direct_native_codex_launch_preserves_account_policy_and_literal_input, release_acceptance_recovers_from_a_revoked_selected_account}` fail inside this checkout because `/Users/jack/src/loopflow/lf.config.local.yaml` routes Codex to `manabot-eng`; both pass from `/tmp`. Unrelated to this diff; CI owns them.
