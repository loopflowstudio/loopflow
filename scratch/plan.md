# yolo is on by default

## Outcome

Agents act without asking permission. That is every launch with no `yolo`
setting, on Claude, Codex, OpenCode and Antigravity. Pi keeps its own tool
policy.

`yolo: false` brings back the vendor's own prompts and configuration **in a
conversation someone attends**. An unattended run never asks, whatever `yolo`
says: nobody is there to answer.

The whole permission model is one sentence and one predicate:

```
skips permissions  =  yolo  ||  nobody is attending
```

| | attended (`lf`, `lf <skill>`, `lf task open`, resume) | unattended (`lf -b`, Task steps, auto, commit and PR text) |
|---|---|---|
| `yolo` unset or true | skips | skips |
| `yolo: false` | vendor's own prompts and config; lf sends nothing | skips |

"Skips" per harness: Claude `--dangerously-skip-permissions`; Codex
`danger-full-access` + `never`; OpenCode allow-all `permissions` (and `--auto`
on its headless run); Antigravity `--dangerously-skip-permissions`.

Who notices: anyone opening an attended conversation, which now starts
without prompts. Unattended runs on Claude, OpenCode and Antigravity are
unchanged. An unattended Codex outside a Task checkout moves from
`workspace-write` to `danger-full-access`.

## Why this model (chosen for review)

Before this change the code holds four permission ideas: the `yolo` setting,
a per-harness "headless floor" that reads the vendor's own config and raises
it, a Task-checkout rule that forces yolo for a headless run in its Task's
checkout, and forced `skip_permissions: true` on commit messages. They differ
by harness: under `yolo: false` a headless Claude bypasses, a headless Codex
gets `workspace-write` outside a Task checkout and full access inside one.

With the default on, all of that only runs when someone has written
`yolo: false`, and the person who writes it wants one thing: to be asked.
Being asked needs someone present. So the floors and the checkout rule
collapse into "unattended never asks", with the checkout precondition
dropped instead of the rule.

Passed over:

- **Keep the floors, delete the Task-checkout rule** (the kickoff plan).
  `yolo: false` then means a different thing on each harness when headless,
  and a headless Codex Task step runs in `workspace-write` with no network:
  it cannot fetch crates, publish a PR or write `~/.lf`. Someone who writes
  `yolo: false` to keep their prompts would find their Codex Task steps
  failing.
- **`yolo: false` means lf sends nothing, attended or not.** Smallest code,
  but every unattended run then depends on the vendor's config allowing
  edits and shell, and the behavior people have today (asked when present,
  never stalled when away) could no longer be written.

What is lost on purpose: `yolo: false` no longer gives a headless Codex
outside a Task checkout the `workspace-write` sandbox, and lf no longer
reads `~/.codex/config.toml` or `.claude/settings.json` to avoid lowering a
more permissive vendor config (it always sends the top level, so there is
nothing to lower). lf has no sandbox of its own; containment is a dedicated
Machine, VM or container, and the docs say so next to the new default. A
Codex-only sandbox setting would be a separate outcome (`codex.sandbox`), not
part of this one.

## Source of truth

- The default: `yolo: bool = "false"` → `"true"` in
  `rust/loopflow-execution/src/config/catalog.rs:276`. `Config::default()` is
  generated from that literal. Every launch path copies `config.yolo` into
  `AgentConfig.skip_permissions` (`prompt/process.rs:210`,
  `agent/session.rs:92`, `harness/claude.rs:751`, `ops/commit.rs:329`); none
  has its own default.
- The rule: one predicate in `loopflow-execution`, on `AgentConfig` taking the
  launch's attended/headless fact (`LaunchKind::mode()` in `harness/mod.rs:257`
  already says which). Each harness asks it and nothing else:
  `claude_options` (`agent/mod.rs:692`, today `claude_skip_permissions(cwd,
  stream, skip)`), `build_codex_thread_start_params` (`agent/mod.rs:795`, also
  feeds `codex_terminal_options`), OpenCode settings (`harness/opencode.rs:402`),
  Antigravity's launch (#1998).
- Settings are `NextLaunch`: a running conversation keeps what it started
  with; a resumed one picks up the new default.

## Built

All three slices are on this branch, stacked on LOO-1013 (#1998, open on
2026-11-20) with `lf sync LOO-1013`; `lf sync main` once it merges.

- `AgentConfig::skips_permissions(Mode)` in `agent/mod.rs` is the one rule;
  the field it reads is `AgentConfig.yolo` (recorded requests keep the
  `skip_permissions` key).
  Claude terminal and Antigravity terminal pass `launch.kind.mode()`, Claude
  stream `Headless`, Codex app-server `Interactive` only with a native client
  attached, Codex terminal and OpenCode `kind.mode()`. Antigravity's headless
  run passes the flag unconditionally (headless by construction).
- Deleted: the Codex/Claude vendor-config readers and floors,
  `is_task_checkout`, `checkout_yolo`, the `batch` parameter of
  `resolve_session_config`, the forced bypass in `ops/commit.rs`. The grep
  below finds nothing.
- Default flipped in `catalog.rs:276` with the new description; `--yolo`
  help in `lf/mod.rs:95`; docs in `docs/config.md`, `docs/security.md`,
  `docs/troubleshooting.md`, `harness/README.md`.
- The launch-policy fixture opened its "attended" terminal as a `Batch`; it
  now says `Chat`, which is what `lf -i` launches.

Must survive: `yolo: false` at every layer and scope (personal, repository,
Wave, skill, Graph step) for attended launches; `LF_YOLO_OVERRIDE`; layer
precedence (`settings.rs:1717-1752`); the worktree `--add-dir` roots for
attended Claude and Codex (`provider_writable_roots`), which matter exactly
when `yolo: false`; Codex auto-approval of app-server requests on headless
drivers (`ApprovalPolicy::AutoApprove`, a different thing: how lf answers a
request, not what it asks the vendor for); Pi untouched.

Checks run in implement: `cargo test -p loopflow --lib launch_policy` pass;
`cargo test -p loopflow-execution --lib -- config:: harness::launch_matrix
agent:: harness::` pass; integration `config_tests:: documented_commands::
settings_launch_tests::` one failure,
`automatic_agent_selection_requires_exactly_one_installed_harness` (agent
discovery, untouched here; likely the macOS PATH trap).

## Remaining

- Gate: rerun that failure from an empty directory; run
  `session_launch_contract`; CI runs the suites.
- `lf sync main` after #1998 merges, then recheck the Antigravity rows.
- Not probed: the Claude root-refusal wording in `docs/troubleshooting.md`
  is from memory (needs a root shell).

## Decisions

**`--yolo` stays, no `--no-yolo`.** It overrides a configured `yolo: false`
for one attended launch. Off is `--setting yolo=false`. The help line says
so: "Skip permission prompts in this conversation, over a configured
`yolo: false`".

**No launch banner.** Each vendor shows its bypass state in its own UI.

**A scoped `yolo: false` on a headless step does nothing**, and the docs say
it in the one sentence about unattended runs. No warning: the setting is
valid and applies the moment someone opens that step attended.

## Absent and error states

- `yolo: perhaps` (invalid): skipped with the existing warning; the launch
  falls to the default, now on.
- Claude refuses `--dangerously-skip-permissions` as root without
  `IS_SANDBOX=1`, and under a managed `disableBypassPermissionsMode`.
  Headless runs already meet both; attended launches now do too.
  `yolo: false` is the way out for an attended conversation, and
  `docs/troubleshooting.md` says so. Not probed (needs a root shell).
- A remote Machine resolves the default with its own `lf`; one on an older
  release keeps the old default until upgraded.

## Surfaces

- CLI: `lf config` shows `yolo` true from the builtin source; `--yolo` help.
- Catalog description (feeds generated reference and schema): "Agents act
  without asking permission; false keeps the vendor's own prompts in
  conversations you attend. Unattended runs never ask. Pi keeps its own tool
  policy".
- `docs/config.md` "Permissions (`yolo`) and worktree sandboxes": the table
  above in two sentences, then worktree access; flag table row at :203.
  `docs/security.md` "Effective action policy": the same two sentences, next
  to "lf has no sandbox of its own".
  `docs/troubleshooting.md`: the Antigravity "Run this command?" entry applies
  only to an attended `yolo: false` conversation; add the Claude root refusal.
  `rust/loopflow-execution/src/harness/README.md` matrix row;
  `rust/loopflow/tests/README.md:103,549`.
- Desktop: no yolo control or wire field. Nothing changes.
- Release notes: one line when the release is cut; no migration.

## Demo

In a repository with no `yolo` setting: `lf -a claude` opens Claude showing
bypass permissions on and runs a shell command without asking.
`lf -a claude --setting yolo=false` opens the same conversation and Claude
asks. `lf -b -a codex --setting yolo=false 'run curl -sI https://example.com'`
still reaches the network. `lf config` lists `yolo` as true from the builtin
default.

## Acceptance check at gate

```bash
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  cargo test -p loopflow --lib launch_policy < /dev/null
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  cargo test -p loopflow-execution --lib harness::launch_matrix < /dev/null
```

Both pass with the four-cell rows on Claude, Codex, OpenCode and Pi (and
Antigravity once #1998 is under the branch). `grep -n
"codex_permission_policy\|claude_skip_permissions\|is_task_checkout" -r rust`
finds nothing.

Review page: `scratch/concept-review.html`.
