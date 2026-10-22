# LOO-554: lf on Linux with Claude, Codex, OpenCode and Pi

Status: both rounds implemented and QA'd on rented machines (ledger
`scratch/loo-554-qa.md`); ready to land, 2026-10-22. Round one ran
2026-10-21; Jack Heart added scope the same day before publishing (round two,
below), built at 34d866b5c3 and proven on a second machine. Jack Heart
directed a fully autonomous run (plan, implement, land), so assumptions in
`questions.md` stand. Jack Heart requested QA, not testing: run lf as a
newcomer on a rented Linux machine with Jack's real logins forwarded through
Loopflow, and fix what it finds at its owners in this branch.

Round three (2026-10-22) is landing plus one cosmetic fix from the round-two
concept review: the earlier Flow stopped after implement when its steps could
not launch (missing managed Claude credential), so nothing was published.
Round four (2026-10-22) restarts the Flow on the same committed tree after
the round-three steps past implement failed the same way; jack@loopflow.studio
verifies again and its implement step found the managed Claude login active
(verified). Round four's compress folded the missing-OpenCode error into
`opencode_export` (85efd23e74). Round five (2026-10-22) resumes after both
Claude accounts hit their five-hour limit during realign: nothing new to
implement, no PR exists yet, and the remaining work is the Landing section
below. Main moved four commits (to ce21904f77, including #1648, LOO-565's
Claude folder-trust change) and merges cleanly. The
connect lines now name the provider by display name (00deba37ad, below). No
other defects are open. PR #1635 merged into main
as 173bb90880, so the launch-output results below now describe this branch on
main, not a combined tree. Sync merged main at ed568b1ff5 (#1638) as
e1bfa5c7e1. The one conflict, the AGENTS.md sentence on generating
`docs/lf-reference.md`, resolved to main's test name
(`reference_matches_the_compiled_cli`) with this branch's "inherited `LF_*`
unset, `< /dev/null`" guidance kept. That test passes on the merged tree, so
the reference needs no regeneration; the branch's own tests pass there too.

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

## Round two: added scope (Jack Heart, October 21; implemented 34d866b5c3)

Jack Heart added four items before publishing: OpenCode credential forwarding
for customers (P8), the `install.sh` maintainer block, the vague missing-`gh`
error, and an end-to-end Codex forwarding proof on a rented machine. Claude's
per-worktree trust prompt moved to LOO-565.

### Demo

From the laptop, with a fresh rented machine added as `m` that has no `gh`:

```bash
lf machine connect m github
# Error: … GitHub CLI (gh) is not installed on m; install it (https://cli.github.com) and run this again
lf machine connect m codex jack@loopflow.studio      # approve in Chrome
lf machine connect m opencode                        # approve OpenCode's device page in Chrome
# Connecting OpenCode Zen jack@… on m
# Connected OpenCode Zen on m: Zen accepted a fresh login for the same OpenCode account as this laptop
lf --machine m -b -a codex : "Reply with PONG"                          # PONG
lf --machine m -b -a opencode:opencode/gpt-5.4-nano : "Reply with PONG" # PONG on a paid Zen model
```

Before the connect, the same OpenCode command on `m` fails with OpenCode's
"Model unavailable: opencode/gpt-5.4-nano". A newcomer's `install.sh` ends
with `Installed to …` and no "Promotion preflight … VERDICT" block (visible
from the first release cut after this lands).

### Risk findings (probed 2026-10-21 on the laptop, OpenCode 2.0.32)

- **OpenCode 2 owns its credentials in its own store**, not the 1.x
  `~/.local/share/opencode/auth.json` that lf reads. `opencode auth
  export [opencode]` prints stored credentials as JSON (`id`,
  `integrationID`, `label`, `active`, `value`); `opencode auth import`
  reads that JSON on stdin, prints "Imported 1 credential", and is idempotent
  by `id` ("skipped 1 already present"). An imported entry on a clean home
  ran a paid model (`opencode/gpt-5.4-nano` → PONG).
- **Jack's laptop has two OpenCode entries**: an inactive API key that Zen
  rejects (`401 api_key_expired`) and the active OpenCode Console OAuth login
  ("Default", `methodID: device`, with refresh grant, expiry and
  `metadata.{email, accountID, orgID}`). lf's 1.x reader still finds the
  expired key in `auth.json`, and `lf account` calls it "cached active".
- **Zen validates any bearer it is sent.** `GET
  https://opencode.ai/zen/v1/models`: no bearer → 200 (public list); bogus
  bearer → 401 "Invalid credential"; expired key → 401 `api_key_expired`;
  the active OAuth access token → 200. That request is the honest check:
  Zen accepted the credential. It names no owner.
- **OpenCode honours `OPENCODE_API_KEY` only without its own login.** Clean
  home + env key → paid model works; clean home + bogus env → "Reconnect
  OpenCode Console to continue"; laptop with its own login + bogus env →
  works (env ignored). lf's stale-key injection is harmless while OpenCode's
  own login exists.
- **Device login runs without a terminal.** `opencode auth login opencode
  --method device --standalone` with stdin closed prints "Enter code:
  XXXX-XXXX" and `https://opencode.ai/console/device?user_code=…&client_id=opencode-cli`,
  then waits for approval. Without `--method` it refuses ("Pass --method when
  running without an interactive terminal").
- **Missing `gh`**: the remote's `inspect` reads `hosts.yml` and passes;
  `receive` then fails spawning `gh auth login` (ENOENT). Locally,
  `remote_command` discards the remote's stderr, so the only text is "machine
  credential command failed … inspect its account status".
- **`install.sh` block**: `release/install.sh` runs the downloaded release's
  `lf install promote`; `promote_published_from_installation` calls
  `render_human` on every promotion. The fix ships with the next release, and
  a dev build cannot exercise `promote` (only a published candidate may
  advance).
- **Codex proof**: `lf machine connect m codex <email>` mints a fresh login in
  a laptop staging home and opens the verified Chrome profile. The two earlier
  requests ended without a login because nobody approved them.

### Alternatives for OpenCode forwarding

| Route | Mechanism | Verdict |
|---|---|---|
| Copy the active entry | `auth export opencode` on the laptop → `auth import` on `m` | Zero-touch, but an OAuth login shares one refresh grant across machines, which `docs/security.md` rules out for Claude, Codex and Linear ("not the laptop's existing refresh credential"). Kept only for API keys, as GitHub tokens are copied. |
| Mint a fresh device login on the laptop | device login in a private staging home, approved in the laptop's Chrome profile, export from staging, transfer, import on `m` | **Chosen for OAuth logins.** The same shape as Claude/Codex/Linear: fresh grant, laptop login untouched, identity checked before transfer. |
| Mint on the machine | run the device login on `m` over SSH, relay the URL | Credential never transits, but needs a streaming remote command beside `remote_command`; no other provider works this way. Rejected. |
| lf-owned `OPENCODE_API_KEY` token | store a ProviderToken on `m`; inject at launch | OpenCode ignores the env key once it has its own login, and an OAuth access token expires without lf refreshing it. Rejected. |

### Chosen approach

**OpenCode forwarding** (`machine_credentials.rs`, `account.rs`):

- `lf machine connect m opencode [email]` (`Provider::OpenCodeZen`; the
  existing parser already accepts `opencode`, `zen`, `oc`). The laptop reads
  `opencode auth export opencode --standalone` and takes the active entry.
  No OpenCode, or a 1.x `opencode` without `auth export` → "OpenCode 2 is not
  installed on this laptop; install it with `npm install -g @opencode/cli`"
  (`opencode_export` raises that one message, naming the laptop or the machine).
  No active entry → "OpenCode has no active login on this laptop; run
  `opencode auth login`".
- Active entry is an **API key** → validate it with Zen and send it as is.
  The success line says Zen accepted the key and no owner was checked.
- Active entry is an **OAuth login** → `fresh_opencode_login` (beside
  `fresh_linear_login`): staging temp home (`HOME` and `XDG_*` pointed into
  it), spawn the device login, read the `…/console/device?user_code=…` URL
  from its stdout (ANSI stripped), open it in the verified Chrome profile
  (`browser_profiles(store, Provider::OpenCodeZen, None, chrome_profile)`),
  wait for exit 0 through `wait_for_browser_confirmation` (the Claude/Codex/Linear
  timeout and heartbeat), export from staging. The fresh
  entry must carry the same `metadata.accountID` as the laptop's active login
  ("fresh OpenCode login belongs to a different account; discarded"), and Zen
  must accept its access token. Then the staging home is dropped.
- `email`, when given, must equal the entry's `metadata.email`; an API key has
  none, so an email with a key fails naming that.
- Transfer: `Credential::OpenCode { login, entry }` where `entry` is the
  exported JSON object, through the existing `receive` path. `login` is the
  OpenCode account id, or the entry id for an API key; the email appears only
  in the "Connecting" line. There is no separate `id` field: it is inside
  `entry`.
- `remote_has` → `inspect opencodezen <login>`: true when `m`'s export holds an
  entry with that id and Zen accepts it.
- Both shared connect lines name the provider through `Provider::display_name()`
  ("Connecting OpenCode Zen jack@… on m", "Connected Codex on mini"); `as_str()`
  stays the stored slug.
- `install` on `m` (under `credential_lock("opencode")`): Zen must accept the
  received entry. Another active `opencode` entry that Zen accepts → "a
  different OpenCode login is already active on this machine; left unchanged".
  A same-`id` entry Zen rejects → `opencode auth logout opencode <id>`, then
  import. `opencode auth import --standalone` with `[entry]` on stdin, its
  output discarded; then the export must show that `id` active (`opencode
  auth switch opencode <id>` when it is not). OpenCode missing on `m` →
  "OpenCode 2 is not installed on this machine; install it with `npm install
  -g @opencode/cli`".
- Zen URL follows the GitHub/Linear test seam (`#[cfg(test)]`
  `LF_TEST_MACHINE_ZEN_URL`); tests put a stub `opencode` on `PATH`, as the
  Codex tests do.

**Remote reason** (`remote_command`): keep withholding the remote's raw
output, but name the remote lf's own `Error: ` line when it printed one
(`machine credential command failed on m: <reason>`). Only lf's authored
message crosses: child output is already discarded in `install`, provider
errors are mapped to fixed strings, and tracing lines are not `Error: `
lines. With no such line (ssh failure), use `os_process::exit_reason` of
stderr.

**Missing gh** (`install`, GitHub arm): a `gh` spawn that fails with
`NotFound` bails "GitHub CLI (gh) is not installed on this machine; install
it (https://cli.github.com) and run `lf machine connect` again". No precheck.

**install.sh block** (`install.rs`): `promote_published_from_installation`
renders the preview only for `--preview`; a refusal already lists its reasons
in the error. `lf install preflight` keeps the full block for maintainers.

**Codex proof**: run on the machine, approve the Codex login in Jack's
verified Chrome profile (see `questions.md`), then a headless `lf -a codex`
turn there and `lf account` listing the login.

### Machine run (one rental, ≤ $1 of the $4.20 left)

1. RunPod CPU pod as in round one (2 vCPU, ~$0.06/h, deadline-delete guard),
   user `qa`, no `gh`. Harnesses at npm latest; OpenCode via `@opencode/cli`.
2. Remote `lf` on `PATH` is this branch's Docker `linux/amd64` build (the
   receive side changed); the laptop runs this branch's build too
   (`remote_command` and the OpenCode source changed).
3. `lf machine connect m github` without `gh` → the new message; install `gh`,
   rerun → connected.
4. Codex: connect, approve, `lf -b -a codex : …` on `m` → answer, `lf account`
   lists `codex jack@loopflow.studio`.
5. OpenCode: paid-model turn on `m` fails first; connect, approve, rerun →
   PONG; rerun connect → no transfer ("already connected" path); laptop's
   `opencode auth list` unchanged.
6. Token-name scan of `/proc/<pid>/environ`, argv and `~/.lf/logs` as in
   round one. Delete the pod, confirm, record spend.

### Exclusions

- lf's local OpenCode account integration stays as is: the 1.x `auth.json`
  reader, `OpenCodeZenBroker` (waits for a 1.x file), `lf account` calling the
  expired key active, and `OPENCODE_API_KEY` injection. Separate Task
  (`questions.md`).
- Pi credential forwarding (Pi owns its credentials); Claude trust (LOO-565).
- Launch-time OpenCode forwarding for `lf --machine m -a opencode`: it has no
  managed account to select; explicit `lf machine connect` only.

### Forbidden outcomes (round two)

- Sending the laptop's own OpenCode OAuth refresh grant to another machine.
- Any credential byte, Zen response body or `auth export` output in stderr,
  logs, captures or notes. Zen's error *message* (e.g. "This API key has
  expired") is fine.
- A remote precheck for `gh` or `opencode` instead of naming the failure.
- Replacing a different, working OpenCode login on the machine.
- A second OpenCode export/import helper outside `machine_credentials.rs`.

### Done when (round two)

- Without `gh`, `lf machine connect m github` names gh and how to install it.
- On a rented machine, a forwarded Codex login answers a headless turn and is
  listed by `lf account` there.
- `lf machine connect m opencode` makes a paid Zen model answer on `m` that
  failed before; the laptop's OpenCode login is unchanged; a second connect
  transfers nothing; a laptop with only an expired key is refused with Zen's
  reason.
- A published promotion prints no preflight block; `--preview` still does.
- Docs (`docs/subscriptions.md`, `docs/security.md`, `docs/getting-started.md`
  machine sections, `lf machine connect` help) list OpenCode and say an
  OpenCode OAuth login gets a fresh device grant while an API key is copied.

### Landing (round three)

Sync with main, gate, publish and land through the Flow. The PR carries both
rounds; the ledger is the QA evidence and the captures under
`scratch/loo-554-qa/` are Jack's demo material. `lf scratch clear` removes
them at submit, so the PR body summarizes the ledger's FAIL→PASS lines.
Follow-ups stay in `questions.md`: OpenCode 2 account reads, OpenCode's
pre-login failure text, Pi using forwarded logins (Jack's call).

### Gate (round two, headless)

```bash
cargo test -p loopflow --lib -- machine_credentials   # opencode install/inspect/refusals, gh NotFound reason, remote Error: line
cargo clippy --all-targets -- -D warnings
```

Expected: pass. The machine run's ledger lines carry the end-to-end proof;
the `install.sh` output is checked on the next release's install
(release-run), since a dev build cannot promote.

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

- **P3/P4 belong to #1635 (decided 2026-10-21; merged 173bb90880).** It
  sends tracing to `logs/lf.log` and deletes the banners. This branch leaves
  `init_tracing`, `terminal_output.rs` banners and the OpenCode banner alone;
  the machine build merged #1635's head, which is now main.
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
(no token env seen on Claude's route), P11/P12 (not exercised). P8 moved
into round two. P3/P4 confirmed on 0.14.14 and gone with #1635, now on main.

Remaining after round one: Codex account checks (round two proves them),
Claude paste/OSC 52 copy-out and OpenCode's own OSC 52 copy are UNKNOWN.

## Concept review (2026-10-21, headless)

The branch's concepts hold: one ordered clipboard source list; `ClipboardContext`
(Off / IfAvailable for `paste` config / Required for explicit `-c`), which matches
the two real ways a person asks for clipboard text; one `exit_reason` formatter
for a provider's last word; the Pi extension's lf path in its spec, not in the
inherited env; relay decided by whether stdout is a terminal.

Resolved (bd449bd264): version skew names `lf install` on both sides,
matching the README's shortest resolving form.

Counterexample kept, accepted: `lf -a claude | tee log` (stdin a terminal,
stdout a pipe) no longer gets the relayed screen. Piping an interactive TUI is
not a supported path; `lf -b … > out` typed at a terminal is, and it now writes
only the answer.

Since resolved by Jack Heart: OpenCode forwarding joined this Task (round
two); Claude's trust prompt went to LOO-565. Pi using forwarded logins stays
an open product call in `questions.md`.

### Round two (2026-10-22, headless)

Reviewed the implemented OpenCode forwarding, the remote `Error:` reason, the
`gh` NotFound message and the `install.sh` preview against the accepted design.

Concepts that hold: lf is a courier for OpenCode's own credential record
(`Credential::OpenCode { login, entry }` carries one `auth export` entry
unchanged, imported by OpenCode's own `auth import`), so lf owns no second
credential shape; the OpenCode account id is the identity (`login`), with the
email only in the "Connecting" line, since an API key names no email; "Zen
accepted it" is the one honesty check on both sides and the success line says
what was and was not verified; a different working login on the machine is
left alone (accepted forbidden outcome); the remote lf's authored `Error:`
line is the only remote text that crosses, which keeps the no-credential-bytes
rule without a precheck.

Settled by the accepted design and implemented (2026-10-22):

- **One provider name across the connect lines.** The machine run printed
  "Connecting OpenCode jack@… on m" and then "Connected opencodezen on m: …"
  (`r2-opencode-connect.txt`): the second line rendered the provider slug,
  which nobody typed and which the design's demo does not show. Both shared
  lines now go through one `connecting` helper and the final line, each using
  `Provider::display_name()`; the hand-written "Connecting OpenCode …" string
  is gone. Result: "Connecting Codex work@… on mini", "Connected OpenCode Zen
  on mini: Zen accepted a fresh login …". No stored string changed; `as_str()`
  stays the stored slug. The design section above now states the `login`
  identity and the absent `id` field as built.

Not changed, with the reason: `opencode_export` treats any failing `opencode
auth export` as "needs OpenCode 2" (a 1.x binary and a broken 2.x export read
the same); distinguishing them would be a version precheck, which the design
rules out. `lf --machine m account` does not list the forwarded OpenCode login
because lf's local OpenCode reads are 1.x; that is the follow-up Task in
`questions.md`, and the round-one demo line claims only Claude and Codex.

Open product question, recorded (not a block for this Task): whether lf should
forward Claude or Codex logins into Pi. Jack Heart's assumption in
`questions.md` (Pi owns its credentials) stands for LOO-554 under the
autonomous-run direction of October 21; the question remains Jack's call for
a later Task.

### Round five (2026-10-22, headless)

Since round two only c84f113161 and 85efd23e74 touched code, both folds with no behavior change (`opencode_export` now raises the missing-OpenCode error itself). No new concept, name or open choice; nothing to implement.

## Check results

Check (2026-10-21, compress): `cargo clippy --all-targets -- -D warnings` clean; `cargo test -p loopflow --lib -- platform::clipboard machine::tests` passed; full suites → gate/CI.
Check (2026-10-21, publish): cargo fmt --check, gate lib tests (5), golden_prompt, clippy -D warnings → pass; full matrix → CI.
Check (2026-10-21, kickoff round two): laptop probes of OpenCode 2.0.32 export/import/device login and Zen `/zen/v1/models` → findings above; plan only, no build.
Check (2026-10-22, implement round two): `cargo test -p loopflow --lib -- machine_credentials` 13 pass; `cargo clippy --all-targets -- -D warnings` clean; machine run → ledger.
Check (2026-10-22, round three through decision): merged tree e1bfa5c7e1; `cargo fmt --check` clean; `cargo test -p loopflow --test integration reference_matches_the_compiled_cli` pass; 17 focused lib tests pass; `git merge-tree HEAD origin/main` (845e8b7c8c, #1647) → 0 conflicts; full matrix → gate/CI.
Check (2026-10-22, implement round three): connect lines use `display_name()`; `cargo test -p loopflow --lib -- machine_credentials` 13 pass; `cargo clippy --all-targets -- -D warnings` clean; machine re-run of the OpenCode line → demo/gate.
Check (2026-10-22, realign round three b): HEAD a87e518b67 (merge of main 91b21dc508); `cargo test -p loopflow --lib -- machine_credentials` 13 pass; `cargo clippy --all-targets -- -D warnings` clean; full matrix → gate/CI.
Check (2026-10-22, kickoff round four): HEAD 83d6c8e453 (plan-only commit over a87e518b67); `git merge-tree HEAD origin/main` → 0 conflicts, 0 commits behind; no build, the round-three-b checks still apply.
Check (2026-10-22, implement round four): HEAD c71dfd5127 differs from 83d6c8e453 only in this plan; `git merge-tree HEAD origin/main` → 0 conflicts, 0 behind; `lf context` scratch 10736/12000 tokens; no code change, the round-three-b checks still apply.
Check (2026-10-22, compress round four): `opencode_export` now owns the missing-OpenCode error (no `Option` layer, `opencode_missing` deleted); `cargo test -p loopflow --lib -- machine_credentials platform::clipboard` 15 pass; `cargo clippy --all-targets -- -D warnings` clean; `lf context` scratch under budget.
Check (2026-10-22, kickoff round five): HEAD 85efd23e74; `git merge-tree --write-tree HEAD origin/main` (ce21904f77, 4 ahead) → clean; `lf context` scratch 10909/12000 tokens; plan-only, compress-round-four checks still apply.
Check (2026-10-22, implement round five): HEAD e3c5166d43 (plan-only over 85efd23e74); origin/main ce21904f77 merges clean; `lf context` scratch 11066/12000; no code change, compress-round-four checks still apply.
Check (2026-10-22, realign round five): HEAD 47b32ac438 (merge of main ce21904f77, compress aaea2a076c); `cargo test -p loopflow --lib -- machine_credentials platform::clipboard` 15 pass; origin/main unmoved; full matrix → gate/CI.
