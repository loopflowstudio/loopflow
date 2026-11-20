# yolo is on by default

## Outcome

A launch with no `yolo` setting skips agent permission prompts, attended or
headless, on Claude, Codex, OpenCode and Antigravity. Pi keeps its own tool
policy. `yolo: false`, `--setting yolo=false` and `LF_YOLO_OVERRIDE=false`
turn it off; an attended conversation then keeps the vendor's own prompts, as
it does today.

Who notices: anyone opening an attended conversation (`lf`, `lf <skill>`,
`lf task open`, a resumed Session). Claude starts with
`--dangerously-skip-permissions`, Codex with `danger-full-access` + `never`,
OpenCode with allow-all `permissions`, Antigravity without "Run this
command?". Headless Claude, OpenCode and Antigravity already skipped prompts;
headless Codex outside a Task checkout moves from `workspace-write` to
`danger-full-access`.

## Source of truth

One row: `yolo: bool = "false"` in
`rust/loopflow-execution/src/config/catalog.rs:276`. `Config::default()` is
generated from that literal (the `definitions!` macro parses it), and every
launch path reads `config.yolo` into `AgentConfig.skip_permissions`:
`prompt/process.rs:210`, `agent/session.rs:92` (resume), `harness/claude.rs:751`,
`ops/commit.rs:329`. No launch path has its own default, so flipping the
literal is the whole behavior change. Settings are `NextLaunch`: running
conversations keep what they started with; a resumed one picks up the new
default.

## Decisions

**The Task-checkout rule goes** (made for review). `run.rs:504-509,621` and
`agent/session.rs:112-122` force `skip_permissions` for a headless run in its
Task's checkout. With the default on, the rule can only fire against a written
`yolo: false`, where it overrides what was written. After this change
`yolo: false` means the same thing in a Task checkout as anywhere else.
`agent::is_task_checkout` has no other caller and goes with it.
Cost, stated in the docs: a headless Codex step under `yolo: false` runs in
`workspace-write`, with no network and no writes outside the checkout, so a
step that fetches crates, publishes a PR or writes `~/.lf` fails there.
Passed over: keeping the rule, which makes `yolo: false` a setting that
headless Task steps ignore.

**The headless floors stay.** `codex_permission_policy` (`agent/mod.rs:575`)
gives a headless Codex at least `workspace-write` + `never`;
`claude_skip_permissions` (`agent/mod.rs:640`) bypasses for a headless Claude
unless its own mode already does; OpenCode's headless run always passes
`--auto` and Antigravity's always passes `--dangerously-skip-permissions`
(#1998). They only matter under `yolo: false`, and there they are what a
headless run needs to finish at all: nobody is present to answer. For Codex
the floor is also the one real choice `yolo: false` buys a headless run, its
sandbox. No code change.

**`--yolo` stays, no `--no-yolo`.** `--yolo` still does something: it
overrides a configured `yolo: false` for one launch. Off is
`--setting yolo=false`. Its help line and the `docs/config.md:203` row say
which way each goes.

**No launch banner.** Each vendor already shows its bypass state in its own
UI; lf's terminal stays quiet.

## Absent and error states

- `yolo: perhaps` (invalid): skipped with the existing warning, so the launch
  falls to the default, which is now on. `settings_launch_tests.rs:50-110` and
  `config/settings.rs:1321-1331` (`yolo: null` → `!resolved.config.yolo`) flip.
- Claude refuses `--dangerously-skip-permissions` as root without
  `IS_SANDBOX=1`, and under a managed `disableBypassPermissionsMode`. Headless
  runs already hit both; attended launches now do too. Not probed here (needs a
  root shell); `docs/troubleshooting.md` names `yolo: false` as the way out.
- A remote Machine resolves the default with its own `lf`; one on an older
  release keeps the old default until it is upgraded.

## Surfaces

- CLI: `lf config` shows `yolo` true from the builtin source; `--yolo` help.
- Catalog description (also feeds generated reference/schema).
- Docs: `docs/config.md` "Permissions (`yolo`) and worktree sandboxes" and the
  flag table at :203; `docs/security.md` "Effective action policy";
  `docs/troubleshooting.md` Antigravity entry (applies only with yolo off);
  `rust/loopflow-execution/src/harness/README.md` matrix row and the
  Antigravity row; `rust/loopflow/tests/README.md:103,549`.
  Both permission sections keep the sentence that lf has no sandbox of its
  own and containment is a dedicated Machine, VM or container, next to the new
  default.
- Desktop: no yolo control or wire field (`swift/` only round-trips config
  text). Nothing changes.
- Release notes: one line when the release is cut; no migration.

## What goes

- `agent::is_task_checkout`, `checkout_yolo` in `run.rs`, the `if batch`
  block in `agent/session.rs`.
- README matrix row "runs as `yolo` in its Task's checkout" and the docs
  bullets "Headless Task runs … run as `yolo`" in `config.md` and
  `security.md`.
- Test rows that pin "unset is off": `config.rs:1153`,
  `config_tests.rs:68`, `launch_policy_tests.rs` (the `(false, &[], false)`
  shape stays valid because it writes `yolo: false`; rows with no setting
  flip), `session_launch_contract.rs`, `settings_launch_tests.rs`.

Must survive: `yolo: false` at every layer and scope (personal, repository,
Wave, skill, Graph step), `LF_YOLO_OVERRIDE`, layer precedence
(`settings.rs:1717-1752`), the headless floors and their tests
(`attended_codex_keeps_its_own_policy_unless_yolo_or_a_boundary_applies`),
Pi untouched.

## Antigravity (PR #1998, LOO-1013)

Open and mergeable on 2026-11-20. It edits the same catalog row, the same
docs paragraphs and the launch matrix (its yolo check becomes
`flag("--dangerously-skip-permissions") == skips`). Implement starts from it:
`lf sync main` if it has merged, else `lf sync LOO-1013`. Its terminal launch
follows `config.skip_permissions`, so the default reaches Antigravity with no
code of its own.

## Slices

1. **Flip the row and see what breaks.** Change the literal, add one test
   through the CLI: with no `yolo` anywhere, `-i` on each harness yields
   `skip_permissions`; `yolo: false` in the repository and
   `--setting yolo=false` each turn it off (extend the harness loop in
   `launch_policy_tests.rs:259`). Run
   `cargo test -p loopflow --lib launch_policy` and
   `cargo test -p loopflow-execution --lib config::`. This shows whether any
   path carries a second default; the failing list is the real set of tests to
   update.
2. **Remove the Task-checkout rule** at both sites with `is_task_checkout`,
   and add the row that shows it: a headless Codex Task step under
   `yolo: false` launches `workspace-write`, not `danger-full-access`. Update
   the launch matrix and its README row.
3. **Docs and help**: the four doc files, catalog description, `--yolo` help.

## Demo

In a repository with no `yolo` setting: `lf -a claude` opens Claude showing
bypass permissions on, and it runs a shell command without asking.
`lf -a claude --setting yolo=false` opens the same conversation and Claude
asks. `lf config` lists `yolo` as true from the builtin default.

## Acceptance check at gate

```bash
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  cargo test -p loopflow --lib launch_policy < /dev/null
```

Passes, including the new unset-is-on rows for Claude, Codex, OpenCode and
Pi (and Antigravity once #1998 is under the branch) and the `yolo: false`
headless Codex Task row.
