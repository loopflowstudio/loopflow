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

## What goes

- `agent/mod.rs`: `CodexSandboxMode`, `CodexApprovalPolicy`,
  `CodexPermissionConfig`, `read_codex_permission_config`,
  `parse_codex_permission_config`, `parse_toml_string_assignment` (if no other
  caller), `merge_codex_permission_config`, `codex_permission_policy`,
  `ClaudePermissionMode`, `read_claude_permission_mode`,
  `claude_settings_paths`, `parse_claude_permission_mode`,
  `claude_skip_permissions` and its "less permissive than Loopflow's
  non-interactive default" warning, `is_task_checkout`. Roughly
  `agent/mod.rs:420-660`, and their tests at `:1600-1680`
  (`parse_codex_permission_config_reads_top_level_policy`,
  `codex_policy_preserves_vendor_grants_and_raises_restrictive_defaults`,
  `parse_claude_permission_mode_reads_default_mode`;
  `attended_codex_keeps_its_own_policy_unless_yolo_or_a_boundary_applies`
  becomes the four-cell table above).
- `run.rs:504-509,621` (`checkout_yolo`) and the `if batch` block in
  `agent/session.rs:112-122`.
- `ops/commit.rs:283` `skip_permissions: true`: the launch is a Batch, so the
  predicate already answers.
- Docs and README text: "raise the floor", "more-permissive vendor configs are
  never downgraded", the separate "Headless Task runs … run as `yolo`" bullets
  in `docs/config.md` and `docs/security.md`. The harness README row "runs as
  `yolo` in its Task's checkout" becomes "never asks permission, whatever
  `yolo` says".
- Test rows that pin "unset is off": `config.rs:1153`, `config_tests.rs:68`,
  `config/settings.rs:1321-1331` (`yolo: perhaps` / `yolo: null` now resolve
  on), `launch_policy_tests.rs`, `session_launch_contract.rs`,
  `settings_launch_tests.rs:50-110`.

Must survive: `yolo: false` at every layer and scope (personal, repository,
Wave, skill, Graph step) for attended launches; `LF_YOLO_OVERRIDE`; layer
precedence (`settings.rs:1717-1752`); the worktree `--add-dir` roots for
attended Claude and Codex (`provider_writable_roots`), which matter exactly
when `yolo: false`; Codex auto-approval of app-server requests on headless
drivers (`ApprovalPolicy::AutoApprove`, a different thing: how lf answers a
request, not what it asks the vendor for); Pi untouched.

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

## Antigravity (PR #1998, LOO-1013)

Open on 2026-11-20. It edits the same catalog row, docs paragraphs and launch
matrix. Implement starts from it: `lf sync main` if it has merged, else
`lf sync LOO-1013`. Its terminal launch follows `config.skip_permissions` and
its headless run always passes `--dangerously-skip-permissions`; both move
onto the predicate.

## Slices

1. **The predicate and the default, shown by the matrix.** Flip the literal,
   add the predicate, route Claude, Codex and OpenCode (and Antigravity when
   under the branch) through it. Extend `harness::launch_matrix` and
   `launch_policy_tests.rs:259` to the four cells on every harness: unset
   attended skips; `yolo: false` attended sends no permission flag or param;
   `yolo: false` headless skips, including a Codex Task step
   (`danger-full-access`) and a Codex `-b` outside a Task checkout. Run
   `cargo test -p loopflow --lib launch_policy` and
   `cargo test -p loopflow-execution --lib config:: harness::launch_matrix`.
   This shows whether one predicate reaches every launch path; a path that
   still asks under headless is a second rule to find.
2. **Delete what the predicate replaced**: the list under "What goes" in
   `agent/mod.rs`, `run.rs`, `agent/session.rs`, `ops/commit.rs`, and the
   tests that pinned the floors.
3. **Docs and help**: the doc files, catalog description, `--yolo` help.

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

## Docs written (write-docs)

Intended behavior, in place: `docs/config.md` (permissions section rewritten
around the attended/unattended table, flag table row), `docs/security.md`
(effective action policy), `docs/troubleshooting.md` (new "Claude refuses to
skip permissions" entry), `rust/loopflow-execution/src/harness/README.md`
(matrix row). They name Antigravity, which lands with #1998. Left to slice 3
because they are code strings: the catalog description and the `--yolo` help.
The Claude root refusal text in troubleshooting is from memory, not probed.
