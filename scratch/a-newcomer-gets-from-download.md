# From install to an open Loopflow

Status: proposed design for LOO-312. Kickoff only; no product implementation or
live Mac acceptance recorded. Baseline: `102522bc25adc146d4b8cc3f343677ed8170f636`.
Research performed 2026-09-26 (run-local date).

## Problem

Kim or Scott should be able to install Loopflow on a Mac without a coding tool,
open it, connect a coding tool, and see one useful next action. The current
instructions assume a working shell PATH, an authenticated coding tool, and a
Git project. The primary window offers no obvious way to acquire those first
two things or open the third.

Jack's accepted direction in `wave/growth/MEMORY.md` makes the Mac app the
primary experience and puts direct AI interaction in its embedded terminal.
This design serves that direction. It does not require Linear, GitHub, a Wave,
or a completed Task to finish setup. LOO-316 owns creating a project and a first
Task from nothing; LOO-306 owns the wider app and terminal walkthrough.

## The demo

In a fresh macOS account with no coding tool or developer package manager,
follow the Mac installation instructions, open Loopflow, choose a coding tool,
install it and sign in from the app. Setup changes to **Open project…**; clicking
it opens a folder chooser in that same window. A new Terminal window also runs
`lf --version` without repairing PATH.

An additional integration check opens an existing local Git test project and
gets a provider response through **New conversation**, without a planning or
GitHub account. That proves that “connected” means usable by Loopflow; it does
not turn an existing project into a prerequisite for reaching the setup finish.

## Approach

### One documented installation path

Use the existing verified full Mac shell installer for this release. The Mac
app remains the working experience; Terminal is used once to install it.
`release/install.sh` already downloads a pinned release, checks digests and the
app signature, and delegates activation to the downloaded `lf install promote`.
Do not build another bootstrapper into Swift.

Change the homepage/download Mac CTA to the Mac installation instructions,
rather than directly to `Loopflow-latest.dmg`. Explain how to open Terminal,
paste the installation command, and then open the installed app. Keep the DMG
available in release assets as a manual distribution artifact; do not present
dragging it into Applications as equivalent to verified machine installation
without a separate proof. This CTA change is a proposal, not an accepted copy
decision from Jack.

Update `docs/getting-started.md`'s opening only, with matching download-page
instructions. Remove the immediate `lf init` step: neither a project nor an
initialized repository is required to install and open the app. Put the
coding-tool setup in the app section, with its separate cost stated beside any
claim that Loopflow is free. Leave the broader docs rewrite to LOO-306/309.

After successful promotion, the installer prints the actual app path and an
executable, correctly quoted `open` command. The documented default path opens
that app explicitly; installation itself does not unexpectedly launch a GUI in
headless use. Linux and `--cli-only` output must not promise an installed app.

For an unwritable default `/Applications`, select and create `~/Applications`.
An explicit `LF_APPLICATIONS_DIR` remains authoritative and reports a concrete
write error. Keep updates at the installed app's recorded location. Reconcile
the target selection in `install/published.rs` with the shell bootstrap so
`lf install` does not later create a second app in `/Applications`. Promotion
and its receipt remain the installation authority. Do not add sudo or relax
signature, digest, migration, or recovery checks.

### Make terminal installation durable

After successful promotion, `release/install.sh` adds one clearly marked,
idempotent PATH block to the default Mac zsh's interactive startup file
(`${ZDOTDIR:-$HOME}/.zshrc`). The block conditionally prepends the actual install
directory. Preserve unrelated content, including a missing final newline;
quote paths with spaces or shell metacharacters as literals. Reinstallation
replaces the owned block rather than appending duplicates. Do not infer
persistent configuration from the installer's current PATH alone.

The installer cannot alter its parent shell. Its output and CLI instructions
must still include an immediate `export PATH=...` command for that shell.
Opening a new interactive zsh session must resolve `lf` automatically. For
other shells, retain explicit shell-appropriate/manual PATH instructions;
automatic configuration in this Task is limited to the default Mac zsh.
Report an unwritable startup file as installed-but-PATH-unconfigured with the
exact repair, without rolling back a valid app installation.

Before making this change, isolate HOME and ZDOTDIR in *all* shell installer
fixtures. They currently inherit the developer's HOME and set only a custom
install directory; new profile writes must never reach the developer's shell.

### Setup belongs in the primary window

Add a setup/empty view at `PodiumView`'s no-selected-repository boundary, after
repository discovery has resolved. Do not replace loading or failed reads with
an empty success. Keep existing repository selection and restored sessions
available to returning users; setup is also reachable from an explicit menu
action and from a failed conversation launch.

For no installed coding tool, explain in one sentence why one is needed. Show
Codex and Claude Code choices with links to their official account requirements
and a plain cost statement. Selection reveals one primary **Install Codex** or
**Install Claude Code** button. Codex may be initially selected, matching the
existing default order; do not claim it is cheaper or better. Recognize an
existing OpenCode installation and preserve its launch path; guided new
installation/sign-in for OpenCode is outside this Task.

Installation and authentication run in an ordinary embedded terminal whose cwd
is the user's home, independent of a repository or Session. Reuse the Ghostty
shell surface and its lifecycle. Do not create a fake repo, Run, Wave, or Task
to own setup. Closing the setup terminal interrupts its foreground operation;
returning rechecks actual state and offers retry. Never interpret dismissal,
a subprocess spawn, or an installer exit alone as successful authentication.

Once a tool is installed, the primary action is **Sign in**. Once connected,
the primary action is **Open project…**, with a brief explanation that a project
is a folder of files and currently must use Git. The chooser reuses
`PortfolioService.addRepo` and `RepoScanner.mainRepository`, then selects that
repository on the current `PodiumModel`. A canceled chooser keeps the action;
an invalid folder explains the problem and allows another selection. Move the
primary Go → Open action onto this same path, rather than opening the older
Wave window. Preserve other windows for their existing uses.

No first-launch-complete boolean. The view derives its next action from current
installation/auth observations and repository selection. Provider probes run
off the main actor, with timeouts, and refresh after an operation or app
reactivation. Failure/unknown has a retry action and is distinct from signed out.
Do not continuously launch auth commands on the existing two-second refresh.

### Reuse installation and authentication owners

Add `lf install agent <codex|claude>` as a narrow installer operation. It invokes
the provider's official native installer only after the explicit UI/CLI action,
streams progress to the terminal, returns failures, and verifies the resulting
executable. Download the script to a temporary file before executing it so a
failed curl cannot become an apparently successful empty pipeline. Provider
updates remain provider-owned. It neither downloads a hand-selected provider
binary nor changes Loopflow's active release. Run this repo-independent branch
without invoking Loopflow release promotion or requiring Git.

Use a small Rust definition for each supported guided tool (executable,
installer URL, shell, documentation URL). Reuse it for missing-tool hints and
the setup status response; Swift must not duplicate download commands or
implement OAuth. Explicitly refresh availability after install: the existing
`check_cli_available` caches negative results for the process lifetime. Prefer
a bounded uncached probe for setup over adding a global cache invalidation bus.

Extend `lf auth status <provider> --json` with a dedicated setup projection:
provider, executable availability, native authentication state
(`active|none|expired|unknown`), and optional error. Existing text output remains
useful to CLI users. Define a Rust/Swift DTO and shared fixture with every field
required or explicitly optional; do not parse prose or silently default absent
fields. This is an observation, not another credential store or a promise of
remaining quota.

Build this observation inside `provider_auth` using the native CLI's status
contract and existing broker code. A cached token alone must not make the UI
claim the executable is installed or usable. Codex currently checks file tokens;
extend its ambient status path to support native `codex login status`, including
Keychain-backed login. Keep token extraction and managed account routing with
their existing owners. Never print credentials or probe real local accounts in
tests. Test signed-out, absent, timeout, and native-auth-without-token-file cases.

Sign-in reuses `lf auth connect <provider>` and its existing browser handoff,
including its fallback for a one-time code. Do not require Chrome profiles,
managed account email routing, API keys, or developer OAuth credentials for the
ordinary ambient login. After sign-in, re-observe native status.

Add an explicit `--default-agent` option to this connect operation for the
setup action. On verified success only, update the existing Home config's
`agent` field, preserving unrelated YAML values and reporting malformed config
without replacing it. This makes a chosen Claude login govern future launches
even when an unsigned-in Codex is also installed. No Swift preference or second
default-selection store. Repository/skill/explicit model precedence remains as
defined by the current configuration resolver; choosing a default cannot
silently override a repository's deliberate setting. Installing alone never
changes the default. Choosing an already connected tool performs the same
verification and config update without forcing logout/re-authentication.

For an implicit launch with no available coding tool, replace the misleading
Codex-only error with both supported setup choices and a link to the setup
instructions. Explicit `-m codex` failures still name Codex. Preserve the current
installed-tool ordering when no explicit setup choice or configuration exists.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Does the shell wall exist? | The real installer, with isolated HOME and stubbed transport/promotion, exited 0 and put a fixture in `.local/bin`. No startup files were written; a new `zsh -lic` reported `lf: not found`. A separate minimal PATH probe failed in both login and interactive zsh; explicit PATH succeeded. | Fix persistence and distinguish the current shell from future shells. This is shell simulation, not clean-account proof. |
| Does the app depend on terminal PATH to find lf? | `LocalWaveAgentLauncher.controlLfPath` resolves the bundled helper; `GUIProcessEnvironment` already adds `.local/bin` and common package-manager directories. | Reuse these paths. Do not replace bundled-helper authority with PATH fallback. |
| Does nothing support sign-in? | `lf auth` already invokes Claude/Codex native login, opens the browser and waits for completion. | Expose the existing path. No second OAuth client or credential store. |
| Is npm required? | Both official provider docs now offer native macOS installers; see sources below. | Install without Node, npm or Homebrew as prerequisites. |
| What happens with no tools? | `default_agent()` falls back to Codex; `missing_agent_message` then gives a Codex/npm-only instruction. Availability checks are cached. | Distinguish implicit absence from an explicit provider request; re-probe after install. |
| Is the claimed empty screen current? | Code: primary launch is `PodiumView` → `WorkspaceNavigator` when no repo is selected. Its menu can contain no repository and no actions. “Author a Wave” belongs to `WorkSurfaceView`; `WavesView` is a different scene. | Change the actual primary boundary. Live confirmation remains required before UI implementation. |
| Does the download button run this installer? | No. `website/content.yaml` points at `Loopflow-latest.dmg`; the full verified installer is a separate path. The DMG bundles lf/lfd, but this research did not prove drag-install activation. | Choose one proven activation mechanism for the documented route; proposed CTA change is explicit. |
| Can a standard account use the current app installer? | The shell installer exits if `/Applications` is unwritable. An explicit target already exists. | Select a user-writable target without sudo and keep later updates at that target. |
| Can setup work with no repo? | Current conversation launch requires repo scope. Native terminal surfaces and provider-auth commands need no project concept. | Give setup a shell surface, not a fabricated conversation/Task. |
| Can readiness reuse auth text verbatim? | No JSON mode exists; service status can prefer a stored token, and Codex's broker uses file token extraction. | Add a typed native-status observation in the existing owner. Do not treat cached token presence as live execution proof. |
| Can existing tests establish the finish? | `uv run pytest python/tests/test_shell_installer.py -q`: 16 passed in 6.13s. Tests stub network, archives and promotion and inherited host PATH/HOME. Existing UI modes explicitly distinguish fixtures from live reads. | Add the missing shell boundary proof; retain a real account trial separately. |

Official sources checked during kickoff:

- [Codex CLI installation](https://learn.chatgpt.com/docs/codex/cli): native
  macOS/Linux installer at `https://chatgpt.com/codex/install.sh`.
- [Codex authentication](https://learn.chatgpt.com/docs/auth): native
  `codex login status` reports the active authentication method.
- [Claude Code setup](https://code.claude.com/docs/en/setup): native installer
  at `https://claude.ai/install.sh`, browser login, and account requirements.

No provider installer, login, credential inspection, machine promotion, or
production app launch was performed during kickoff. The run explicitly has no
rendering environment. The installed app's presence is not a live observation
of this branch or a clean account. No clean Mac account was created.

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Repair docs and add Open project only | Small; leaves provider install and auth in separate websites/terminals. | Does not take responsibility for getting an absent coding tool connected or detecting a failed setup. |
| DMG-first self-installing app | Best eventual download experience; can hide even the initial Terminal step. | Not selected here: activation, app restart during promotion, bundled-helper registration, and recovery would need another bootstrap entry point plus a real clean-account proof. Keeping that unresolved dependency while claiming a shorter path would hide work. |
| Existing full installer plus native app setup | One initial Terminal action; existing release activation and provider auth retain ownership. | Chosen. Carries all three walls through to an observable next action without inventing project creation. |

## Key decisions

- The finish is an open, connected app with **Open project…**, not a completed
  Task. This matches LOO-312 and does not borrow unfinished LOO-316 behavior.
- Use the full installer as the documented entry; do not imply that changing
  shell PATH repairs unproven DMG activation. Reconsider this choice during
  design review if direct-DMG installation is a binding requirement.
- Selecting a coding tool must affect actual future launches. Write the one
  existing config authority only on verified success.
- Sign-in is not entitlement or quota verification. A real conversation is the
  integration proof; the UI must preserve a provider's actionable rejection.
- Wild success: a newcomer returns from browser sign-in to a single useful
  action and later opens a terminal where lf already works.
- Wild failure: setup reports success from a cached token, starts the wrong
  provider, or only works on a maintainer's machine. The native-status,
  configuration-precedence and clean-account proofs specifically cover these.

## Scope

- In scope: documented full Mac install route; PATH persistence; writable app
  destination and update consistency; guided Codex/Claude installation and
  existing auth; primary-window setup and project picker; missing-tool errors;
  focused docs/download changes and proofs.
- Out of scope: project creation, Git initialization, first local Task, Linear
  onboarding, GitHub delivery, Wave creation, flow visualizations, broad doctor
  dependency cleanup, general docs rewrite, pricing promises, and a new DMG
  self-bootstrapper. No changes to the Wave's paused state or planning records.

## Done when

1. In an actual fresh macOS account, start with no lf, coding tool, npm,
   Homebrew, repository, or saved app preferences. Follow the exact documented
   installation route through opening the app. Capture OS/account type, release
   or candidate identity, install destination, each action, and exact failures.
   A simulated HOME on a developer account does not satisfy this check.
2. Complete native installation and real browser sign-in for one supported
   provider. Loopflow then offers **Open project…**; the chooser works and
   cancellation preserves the action. Repeat the provider path for the other
   guided provider in a separate clean state before claiming both are supported.
3. Close/reopen the app and a Terminal window. State is re-observed, not remembered
   as a success boolean, and `lf --version` resolves. A repeat install produces
   no duplicate PATH block; an update retains the same app destination.
4. Open an existing local fixture project and send a harmless prompt through
   the real app conversation launch. Observe a provider reply. No Linear or
   GitHub connection is needed. Record CLT/Git availability separately: this
   follow-on proof must not introduce a hidden prerequisite for opening setup.
5. Focused automated proofs cover the owned shell block, escaped paths,
   unwritable profile, repeat install, CLI-only behavior, target selection,
   status failures, installation failure/retry, provider choice and config
   precedence, same-window repository selection, and retained returning-user
   navigation. Stubs prevent network/auth side effects and assert visible
   outcomes. DTO fixtures round-trip in Rust and Swift.
6. Run affected suites once at gate: installer pytest; focused Rust installer,
   provider-auth, config and agent tests; affected Swift model tests and hosted
   UI tests; download-page tests. Run cargo fmt and clippy for Rust edits and
   the Swift boundary check. Use TESTING.md's current build/test commands.
7. Verify public instructions against the released artifacts after publication.
   A local candidate trial proves that candidate only, not the public channel.

## Forbidden outcomes

- Declaring clean-account or live UI acceptance from these code reads, fixtures,
  a screenshot of a different build, or a HOME override.
- Writing the current Mac walkthrough from code before opening the running app.
- Changing only WavesView while leaving the primary no-repository view stranded.
- Calling a downloaded/installed binary authenticated, parsing auth prose, or
  making a failed readiness query look like a successful empty state.
- Adding another credential store, default-agent preference, repository list,
  installation authority, or onboarding-complete flag.
- Auto-installing a provider, signing in, or changing defaults merely on launch.
- Requiring npm, Homebrew, Rust, uv, Doppler, Linear, GitHub, or a project just to
  open and set up Loopflow on the documented Mac path.
- Replacing user shell configuration, writing to the host from tests, requesting
  sudo, or bypassing release validation to make the demo pass.
- Presenting Open project as project creation or claiming a first Task is done.

## Internal slices

1. **Observe and repair installation.** Record the primary window in a running
   baseline app on a fresh account before UI edits. Isolate installer tests;
   implement PATH persistence, writable app destination/update consistency and
   accurate completion instructions. Prove actual fresh-shell command discovery.
2. **Connect a coding tool.** Add the small native installer operation and typed
   native-status projection in existing owners, reuse login, persist explicit
   default selection, and fix implicit no-tool diagnostics. Prove missing →
   installed → connected and failure/retry without a repo.
3. **Carry setup through the primary window.** Reuse embedded shell and repo
   selection, implement setup actions and rechecks, preserve existing sessions.
   Prove the provider reply through a real local project.
4. **Close the public path.** Update the download CTA and narrow docs, perform
   the clean-account acceptance and public-channel trial when released. Keep
   the entire target above intact while implementing these internal cuts.

## This slice

Kickoff has produced the design and baseline evidence. The first executable cut
is slice 1: isolated installer fixtures and durable zsh command discovery.
Focused proof: real `release/install.sh` with fixture transport/promotion followed
by a new interactive zsh resolving the fixture from the chosen install directory.
Repeat it with a spaced home and twice-installed profile. This deliberately
proves shell behavior only; obtain the live baseline before proceeding to UI
implementation and before writing a current-state walkthrough.

## Slice ledger

| Evidence | Result | Limit |
|---|---|---|
| Initial branch inspection | Clean at supplied base; scratch contained only `.gitkeep`; repo guide and growth memory read. | No previous design to reconcile. |
| Minimal environment zsh probe | Login and interactive shells omitted `.local/bin`; explicit PATH resolved the fixture. | Developer OS, temporary HOME, stand-in executable. |
| Existing shell installer tests | 16 passed, 6.13 seconds. | Simulated release transport/promotion; no new-shell test. |
| Real installer script with isolated fixture transport | Exit 0; installed fixture; no profile files; new login shell could not find lf. | CLI-only simulation; app and machine activation stubbed. |
| Primary-window and auth source review | Corrected the assumed UI location and found reusable auth plus native-status gaps. | No rendered/live app observation. |
| Simulated design review | Added DMG-vs-installer distinction, safe test HOME isolation, same-window opening, and provider default persistence. | These are proposed changes, not implementation evidence. |

## Measure

The chapter supplies no metric target. Record the number of manual PATH repairs,
package-manager prerequisites, and dead-end screens along each clean-account
trial. The acceptance target is zero for each, with one obvious next action at
the endpoint. Capture elapsed setup time excluding account purchase/creation as
descriptive evidence, without inventing a chapter KR or a speed promise.
