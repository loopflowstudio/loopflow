# LOO-554: lf on Linux with Claude, Codex, OpenCode and Pi

Status: implemented and QA'd on a rented machine, 2026-10-21; Jack Heart
directed a fully autonomous run (plan, implement, land) the same day, so
assumptions in `questions.md` stand. Ledger: `scratch/loo-554-qa.md`. Jack Heart requested QA, not testing: run lf as
a newcomer on a rented Linux machine with Jack's real logins forwarded through
Loopflow, and fix what it finds at its owners in this branch.

## Problem

A person on Linux, in tmux over SSH, should get the same deliberate experience
with any of the four harnesses: paste works in and out of the native composer,
their own login and chosen model reach the agent, and the terminal shows only
the agent. Nobody has run all four there with real accounts. PR #1635 proved
tmux hosting and Claude hooks; everything else is unobserved.

## Demo

On the rented machine, inside `tmux new -s qa`:

```bash
lf -a codex                     # Codex composer, no Loopflow lines above it
# copy a multi-line Unicode block in tmux copy-mode, then
lf -c -a claude:sonnet          # first turn contains that block; /status shows jack@loopflow.studio, sonnet
lf -a pi                        # Pi with its own configured model
lf account                      # forwarded claude and codex logins listed
```

Jack's eyes are needed only for: approving Claude's browser login once, and
judging the captured screens (`scratch/loo-554-qa/*.txt`) as the experience.

## Findings before renting (code reading, 2026-10-21)

Predicted defects; outcomes are in "Outcome" below.

| # | Finding | Owner |
|---|---------|-------|
| P1 | `clipboard::read` falls through only when a tool is missing; an installed `xclip` with no display ends the search. No tmux-buffer source. | `platform/clipboard.rs` |
| P2 | Explicit `lf -c` with nothing readable launches silently without clipboard text. | `prompt/mod.rs` |
| P3 | Every native launch prints `Loopflow diagnostics: <path>`; OpenCode adds its own. | #1635 |
| P4 | `init_tracing` writes WARN to stderr. | #1635 |
| P5 | Remote `lf --machine m -a pi …` parses Pi as an account `Provider`. | `machine_credentials.rs` |
| P6 | `-a opencode:gpt-5` (no `provider/`) is dropped silently. | `harness/opencode.rs` |
| P7 | Missing-agent fallback says `-m claude`; the flag is `-a`. | `agent/mod.rs` |
| P8 | `lf machine connect` cannot forward the stored OpenCode Zen credential. | `machine_credentials.rs` |
| P9 | lf's OpenCode hint (`npm install -g opencode-ai`) installs 1.19.3; lf targets 2.x. | `agent/mod.rs` |
| P10 | Stored tokens (`GH_TOKEN`, `CLAUDE_CODE_OAUTH_TOKEN`) may enter agent env. | `provider_auth/mod.rs` |
| P11 | Claude Task-worktree sandbox needs bubblewrap and socat on Linux; lf names neither. | `agent/mod.rs` |
| P12 | Pi and OpenCode first turns travel in argv (visible in `ps`, capped at 122,880 bytes). | `agent/session.rs` |

Constraints that shaped the run:

- **P3/P4 belong to #1635 (decided 2026-10-21).** Its head e44ab917 sends
  tracing to `logs/lf.log` and deletes the banners. This branch leaves
  `init_tracing`, `terminal_output.rs` banners and the OpenCode banner alone;
  the machine build merged #1635's head so output checks measured the
  combined result. #1635 was still open on 2026-10-21, so until it lands the
  launch-output PASS lines describe the combined tree, not this branch alone.
- tmux `lf open` routing belongs to #1635; reconnecting here used `lf resume`
  from a second tmux session.
- Pi and OpenCode run with their own credentials; lf's part is its messages.
- Touch only the pod LOO-554 created; RunPod holds an unrelated pod.

## How the run went

One RunPod CPU pod (2 vCPU / 4 GB, $0.06/h; larger shapes had no capacity),
Ubuntu 24.04, non-root user `qa`, a deadline-delete guard, explicit delete at
the end. Baseline: published 0.14.14 from `install.sh`. Branch builds came
from Docker `linux/amd64` on the laptop (the pod OOM-killed `cargo build
--release`), run under a separate `LF_HOME`. Harnesses: latest npm release of
each (see `questions.md`). A detached tmux server driven by `send-keys`,
`capture-pane` and `pipe-pane`, plus provider transcripts, produced one ledger
line per check in `scratch/loo-554-qa.md`; captures are in
`scratch/loo-554-qa/`.

## Scope

In: the four harnesses on Linux in tmux, interactive and headless; paste in
and out; account and model forwarding; terminal output; the fixes above.

Out: machine rental as a product (LOO-526), tmux `lf open` routing (#1635),
host status fidelity (LOO-422), Herdr/cmux polish (LOO-423), Windows/WSL,
OSC 8 links, `lf-<sha>` argv[0] naming. No cmux or Herdr code.

## Forbidden outcomes

- A second clipboard module or a Linux-only reader beside `platform::clipboard`.
- Re-implementing #1635's tracing sink or banner deletion in this branch.
- A clipboard fallback that silently drops explicit `-c`.
- Token values in any LOO-554 capture, note or log.

## Done when

- Every Area × harness cell has a line in `scratch/loo-554-qa.md`: PASS, a
  FAIL fixed in this branch and re-run to PASS, or UNKNOWN with its reason.
- On the machine with the branch build, an interactive launch for each harness
  shows no Loopflow line before the native UI, and `lf -b` stderr is empty on
  success.
- `lf -c` inside tmux with no X display delivers the tmux buffer to the first
  turn. With no source at all, it refuses before launch.
- The rented pod is deleted and confirmed gone. Spend is recorded and under
  the $4.30 remaining of Jack's $5 cap.

### Gate (headless, macOS and Linux CI)

```bash
cargo test -p loopflow --lib -- platform::clipboard        # failing xclip falls through; no source names each attempt
cargo test -p loopflow --lib -- machine_credentials::tests::remote_pi_launch_prepares_no_account
cargo test -p loopflow --lib -- harness::context::tests::pi_extension_names_lf_without_an_inherited_lf_bin
cargo test -p loopflow --lib -- machine::tests::release_numbers_order_by_component
cargo clippy --all-targets -- -D warnings
```

Expected: all pass. Linux behavior is proven by the QA ledger plus Linux CI
clippy/tests. Gate does not rent a machine.

## Delete — do not maintain

- `clipboard::try_command`'s spawn-only fallthrough is replaced by one ordered
  source list in `platform/clipboard.rs`; no second reader.
- The `-m claude`/`-m codex` hint text in `agent/mod.rs`.
- `clipboard::write`, which had no callers (deleted).
- Duplicate last-stderr-line formatting in `codex.rs` and `run.rs`; both use
  `os_process::exit_reason`.

## Outcome (2026-10-21)

Fixed in this branch, each re-run on the machine unless noted:

- P1/P2 clipboard: one ordered source list (xclip, xsel, wl-paste, then the
  tmux buffer inside tmux); explicit `-c` with nothing readable refuses before
  launch naming each source. `paste` config stays silent.
- P5 remote `-a pi` prepares no account (unit test; not run remotely).
- P7 hint flag; P9 OpenCode hint names `@opencode/cli`.
- Found on the machine: Pi's lf extension read `LF_BIN` from its environment,
  so `lf -a pi` exited at startup on any fresh machine; Codex refused lf's
  0775 socket dir under Ubuntu's umask 002, so every Codex launch failed;
  `lf -b … > out` typed in a terminal captured relay screen sequences; remote
  commands could not find npm-prefix agents (Codex login forwarding failed);
  Codex early exits and headless agent failures hid the provider's reason;
  declining Claude's trust prompt blamed "another client"; version skew told
  you to update the newer side; free text as a skill name never mentioned
  `lf : <message>`.

Refuted or not built: P6 (already refused by `validate_agent_policy`), P10
(no token env seen on Claude's route), P11/P12 (not exercised), P8 (deferred,
see questions). P3/P4 confirmed on 0.14.14 and gone with #1635 merged; they
stay on main until #1635 lands.

Remaining: Codex account-dependent checks (no login reached the machine),
Claude paste/OSC 52 copy-out and OpenCode's own OSC 52 copy are UNKNOWN; demo
steps for Jack. Claude's per-worktree trust prompt is an open product call
(`questions.md`).

## Concept review (2026-10-21, headless)

The branch's concepts hold: one ordered clipboard source list; `ClipboardContext`
(Off / IfAvailable for `paste` config / Required for explicit `-c`), which matches
the two real ways a person asks for clipboard text; one `exit_reason` formatter
for a provider's last word; the Pi extension's lf path in its spec, not in the
inherited env; relay decided by whether stdout is a terminal.

Next implement direction:

- **One name for the update command.** Version skew tells you `lf self install`
  for this machine and `ssh … 'lf install'` for the remote. README and user
  guidance use `lf install` (shortest resolving form). Make the local branch of
  `report_version` say `lf install` too.

Counterexample kept, accepted: `lf -a claude | tee log` (stdin a terminal,
stdout a pipe) no longer gets the relayed screen. Piping an interactive TUI is
not a supported path; `lf -b … > out` typed at a terminal is, and it now writes
only the answer.

Not blocking this Task (Jack Heart directed a fully autonomous run; the accepted
outcome does not depend on them): Claude's per-worktree trust prompt, Pi using
forwarded logins, and OpenCode Zen forwarding stay open product calls in
`questions.md` for separate Tasks.

## Check results

- `cargo clippy --all-targets -- -D warnings` clean; `cargo test --lib -- platform::clipboard harness::codex lf::commands::run machine_credentials machine::tests harness::context harness::pi` passed (compress pass); realign ran no tests (npm latest versions re-checked); full suites → gate/CI.

Check (2026-10-21): version skew names `lf install` on both sides; `cargo test -p loopflow --lib -- machine::tests` 2 passed, `cargo clippy --all-targets -- -D warnings` clean.
Check (2026-10-21, compress): clipboard attempts reuse `os_process::exit_reason`; `cargo test -p loopflow --lib -- platform::clipboard` 2 passed, clippy clean.
