# LOO-565: Open Claude in lf-created worktrees without a folder-trust prompt

Status: implemented 2026-10-21; demo/review (real Claude pty on macOS and fresh Linux) remains. Product decision accepted by Jack Heart
on October 21: lf pre-accepts Claude's folder trust for worktrees lf itself creates.

## Problem

Launching Claude interactively in a checkout lf just made (`<repo>.agent-<hash>`,
a Task checkout, `lf wt create`) shows Claude's "Quick safety check: Is this a
project you created or one you trust?" dialog. Its preselected answer is
**No, exit**, so a newcomer who presses Enter leaves the session before it
starts. Found in LOO-554's Linux QA on a fresh machine. Jack's laptop never
shows it because `/Users/jack/src/loopflow` is already trusted (see below).

## Demo

On a machine (or `HOME`) with no Claude trust for the repo:

```
cd ~/src/kata && lf          # moves to kata.agent-<hash>, Claude opens straight into the conversation
cd ~/src/kata && claude      # main checkout lf did not create: Claude's normal trust dialog
git worktree add ../kata.mine && cd ../kata.mine && claude   # hand-made worktree: normal dialog
```

`~/.claude.json` gains exactly one entry, `projects["<realpath of kata.agent-…>"].hasTrustDialogAccepted = true`.

## Risk findings (Claude Code 2.1.307, probed 2026-10-21)

Probes ran the real `claude` binary in a pty against a scratch
`CLAUDE_CONFIG_DIR`, a scratch repo and its linked worktrees.

- **Trust lives only in the global config file.** `projects[<path>].hasTrustDialogAccepted`
  in `<CLAUDE_CONFIG_DIR>/.claude.json`, else `~/.claude.json`; a non-empty legacy
  `<claude dir>/.config.json` wins when present (`<claude dir>` = `CLAUDE_CONFIG_DIR`
  or `~/.claude`). `--settings` files carry no trust. There is no CLI flag for
  interactive session trust; `-p`/print mode trusts its session implicitly, and
  `CLAUDE_CODE_SANDBOXED` trusts everything (rejected: it changes far more than
  trust). The Task's fallback clause applies: lf records only the trust entry for
  the exact lf-created path, and the PR must say so.
- **Lookup for a linked worktree**, in order: the canonical main-checkout key,
  then a walk up from cwd that stops at the worktree's own git root.
  | Trusted key | Worktree prompts? |
  |---|---|
  | none | yes |
  | main checkout | no |
  | exact worktree path | **no** |
  | parent dir (`…/src`) | yes — walk-up no longer crosses the git root |
- **Claude's own "Yes" writes the main-checkout key**, trusting the main checkout
  and every sibling worktree. lf's per-path entry is strictly narrower than what
  the dialog itself does.
- So repeated prompts come from never answering Yes (default exits), or from
  launches whose config dir differs from where Yes was recorded: isolated account
  homes (`LF_ACCOUNT_ISOLATION=isolated` sets `CLAUDE_CONFIG_DIR` to
  `<LF_HOME>/accounts/…`) and fresh `HOME`s. Trust must therefore be written into
  the config **the launched child reads**, decided at launch, not at creation.
- **Concurrent writers.** Running Claude processes rewrite the whole file. Claude
  takes `proper-lockfile` on `<config>.lock` (an mkdir lock, 10 s stale) and
  watches the file for external refresh. lf takes the same lock and replaces the
  file atomically, so neither side loses the other's update.
- Keys are realpaths (`/tmp/…` is stored as `/private/tmp/…` on macOS).

## Chosen approach

Two small pieces with one writer each.

1. **Provenance at creation.** `git::worktree_add_inheriting` — the single path
   every lf worktree creation takes (Task placement, agent worktrees,
   `lf wt create`, Wave/Project runtimes) — writes an empty marker file
   `loopflow-created` at `git rev-parse --git-path loopflow-created`, i.e. inside
   the worktree's own admin dir (`<common>/worktrees/<name>/`). It is invisible to
   the working tree, and `git worktree remove`/`prune` deletes it with the worktree.
   A main checkout or hand-made worktree never has one.
   `git::lf_created_worktree(cwd) -> Option<PathBuf>` returns the canonical
   worktree root when the marker is present.
2. **Trust at launch.** New `harness/claude_trust.rs`:
   `accept_created_worktree(command: &std::process::Command)` reads the command's
   cwd and its effective `CLAUDE_CONFIG_DIR`/`HOME` (reuse `provider_account::launch_env`,
   made `pub(crate)`), and when the cwd is lf-created and the resolved config lacks
   `hasTrustDialogAccepted: true` for that key, it takes `<config>.lock`, re-reads,
   sets the one field (creating `projects` or the project object if needed and
   preserving every other field), writes a temp file beside it and renames it over
   the original (mode 0600), then releases the lock. Already-trusted configs are
   read once and never rewritten.
   Called at the two interactive Claude launch sites after the account route has
   set the child's environment: `agent/mod.rs` run_agent (after `launch_as_blocking`)
   and `lf/commands/util.rs` native launch (after `launch_as_blocking`). The headless
   stream harness (`harness/claude.rs`) runs in print mode, which Claude trusts per
   session; it is left alone.
3. Update the `harness/context.rs` module doc: lf replaces no hook trust; folder
   trust is granted only by `claude_trust` for lf-created worktrees.

### Absent and error states

- Not a git checkout, main checkout, or worktree without the marker: do nothing.
- Config file missing (fresh machine, Claude never run): create it with only the
  `projects` entry; Claude still runs its own onboarding.
- Config unreadable, not a JSON object, or `projects` not an object: leave it
  untouched, `tracing::warn!` to `lf.log`, launch continues (Claude shows its
  normal dialog). Never clobber a file lf cannot parse.
- Lock held past ~2 s by a live writer: skip with a warn; stale lock (>10 s mtime)
  is removed and retried, matching proper-lockfile.
- `CLAUDE_CODE_IGNORE_PERSISTED_TRUST` set: Claude ignores the record; nothing to fix.
- No terminal output in any case (quiet-terminal rule).

## Alternatives considered

- **Trust the main checkout key.** One write covers every worktree, but also trusts
  the main checkout and hand-made worktrees. Violates acceptance. Rejected.
- **Write trust at creation time.** No marker needed, but the creating process may
  use a different config dir than later launches (isolated accounts, rotation,
  another `HOME`), and pre-existing worktrees never get it. Rejected for launch-time.
- **Infer provenance from path/branch naming** (`<repo>.<segment>`, `<user>/<segment>`).
  Not proof; a person can make the same names by hand. Rejected.
- **Record provenance in SQLite (Task checkout rows).** Covers Tasks but not
  agent worktrees or `lf wt create`, and adds a store read to every launch. Rejected.
- **`CLAUDE_CODE_SANDBOXED=1` per launch.** Non-global, but it signals a sandboxed
  host and alters behaviour beyond trust. Rejected.

## Scope

In: Claude, interactive launches, macOS and Linux, worktrees created after this
lands. Shared and isolated account homes.

Out:
- Backfilling markers on worktrees created before this change (one-time prompt;
  a single Yes there trusts the main checkout anyway).
- Codex/Pi/OpenCode project trust (Tasks run on Claude only).
- Claude's `bG()` config-name suffix used by non-default OAuth environments.
- Removing trust entries when a worktree is deleted. Stale keys for absent paths
  are inert and Claude's own dialog leaves the same residue.

## Forbidden outcomes

- Any write to global Claude settings other than the single
  `projects[<exact lf-created worktree>].hasTrustDialogAccepted` field (no hooks,
  no `settings.json`, no main-checkout or parent keys).
- A second provenance record or a creation-time trust writer beside the launch one.
- Rewriting the config when the entry is already true.

## Done when

- `lf` launching Claude in a worktree lf created opens straight into the
  conversation with no trust dialog, on macOS and on a fresh Linux machine.
- The main checkout and hand-made worktrees still show Claude's dialog.
- The only change in Claude's config is the one exact-path trust entry; every
  other field survives byte-for-byte in value.

## Acceptance checks (gate, headless)

- Unit, `harness/claude_trust.rs` with temp `HOME`/`CLAUDE_CONFIG_DIR` and real
  git repos: lf-created worktree gains the exact realpath key and keeps sibling
  fields; main checkout and `git worktree add` worktree leave the file untouched;
  missing file is created; invalid JSON is left as-is; legacy `.config.json` wins;
  an isolated `CLAUDE_CONFIG_DIR` receives the entry in its own `.claude.json`;
  already-trusted config is not rewritten (mtime unchanged).
- Unit, `git`: `worktree_add` leaves the marker outside the working tree. The
  unmarked cases are proven once, through the config, in `claude_trust`.
- Integration, `tests/context_launch_tests.rs`
  `claude_opens_lf_created_worktree_already_trusted`: stand-in
  `claude` copies `<CLAUDE_CONFIG_DIR>/.claude.json` at startup; launching through `lf` in an
  lf-created worktree shows the trust entry already present.
- Command: `env -u LF_HOME … cargo test -p loopflow claude_trust context_launch < /dev/null` → pass.

## Demo/review (people, not gate)

- Real Claude pty probe (`python3 -I probe.py` pattern: scratch `CLAUDE_CONFIG_DIR`,
  look for "Quick safety check") on the laptop and on a rented fresh Linux box
  (RunPod, per the Linux QA setup), launching via the branch `lf`.
- PR body states that lf writes Claude's global `~/.claude.json` trust entry for
  the exact lf-created path only, because Claude has no other trust record.

## Checks

- Kickoff: real-Claude pty probes of the four trust cases and of accept-in-worktree → findings above.
- Implement: `cargo test -p loopflow --lib -- claude_trust lf_created git_worktree_add` (8) and `--test integration -- claude_opens_lf_created_worktree_already_trusted` → pass; clippy `--all-targets -D warnings` clean. Gate owns broader suites.
- Compress: dropped the duplicate git unmarked-worktree test and a redundant `create_dir_all`; `cargo test -p loopflow --lib -- claude_trust git_worktree_add` → 7 pass.
- Realign: plan matches the diff and origin/main (54da279a8b) adds nothing; no rerun. Demo/review owns the real-Claude pty probes.
