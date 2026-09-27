# From install to an open Loopflow

Status: interactive review decisions recorded; ready for Jack's review completion.
Product direction is confirmed below; implementation details and missing proof
are identified separately and are not claims of acceptance.
No product implementation or live Mac acceptance recorded.
Baseline: `102522bc25adc146d4b8cc3f343677ed8170f636`.
Research performed 2026-09-26 (run-local date).

## Problem

Kim or Scott should be able to install Loopflow on a Mac without a coding tool,
open it, connect a coding tool and GitHub, and see one useful next action. The current
instructions assume a working shell PATH, an authenticated coding tool, and a
Git project. The primary window offers no obvious way to acquire those first
two things or open the third.

Jack's accepted direction in `wave/growth/MEMORY.md` makes the Mac app the
primary experience and puts direct AI interaction in its embedded terminal.
This design serves that direction. It does not require Linear, a Wave, or a
completed Task to finish setup. Jack's review direction makes Linear optional
sharing across the product, not merely optional during setup. Jack confirmed
early GitHub sign-in as part of initial setup, before **Open project…**.
LOO-316 owns creating a project and a first Task from nothing; LOO-306 owns
the wider app and terminal walkthrough.

## Interactive review direction

Jack said: "not require linear at all and just have it be an optional sharing
mechanism." The intended local experience must therefore include ordinary
planning and Tasks without Linear, not just a direct-conversation exception.
This is a product direction; current init instructions still describe
Linear-backed Tasks and planning. This design must not claim that removing a
sign-in prompt removes the underlying dependency. LOO-316 is the existing
related Task for first local work; the complete planning/storage change and
its ownership still need to be resolved. No planning records were changed.

After considering requiring GitHub, Jack confirmed: "make sure theyre signed in
asap" in response to initial setup versus first publication. GitHub sign-in
belongs in initial setup, before **Open project…**; do not postpone it until
delivery. Reuse existing `lf auth github` and account for its `gh` executable
dependency on a clean Mac. Verify and reuse an existing sign-in. Authentication
does not authorize creating repositories or publishing code automatically.

See [account requirements review](account-requirements-review.md) and
[init review](init-setup-review.md). Jack chose "drag to app -> open" as the
primary acquisition path. See [DMG review](dmg-first-launch-review.md). Jack
prefers scoped sessions and is content to avoid an init conversation here.
Initial setup uses app actions; a repo-wide init is a separate possibility,
not part of this Task. The mechanism for installing `gh` is an implementation
detail to resolve under the no-package-manager constraint.

## The demo

In a fresh macOS account with no coding tool or developer package manager,
download the DMG, drag Loopflow into Applications and open it in Finder, without
running a Terminal command. First launch activates the installed app through
the existing installation authority. Choose a coding tool,
install it and sign in from the app, then connect GitHub. Existing verified
sign-ins need no repeat login. Setup changes to **Open project…**; clicking
it opens a folder chooser in that same window. A new Terminal window also runs
`lf --version` without repairing PATH.

An additional integration check opens an existing local Git test project and
gets a provider response through **New conversation**, without a Linear
account, with GitHub connected during setup. The response
proves that “connected” means usable by Loopflow; it does
not turn an existing project into a prerequisite for reaching the setup finish.

## Approach

### Existing `lf init` and the setup boundary

Interactive review inspected `lf init` after Jack asked whether kickoff had
considered it. It is an agent-run skill, defined in
`rust/loopflow/src/engine/builtins/ops/skill/init.md`, not a standalone bootstrap
command. It already discovers installed agents and account state, guides
connections and configuration, and ends with an evidence-backed next command.
It supports direct skills without Linear, although its opening prioritizes
Waves, shared planning, and delivery and offers the desktop only at the end.

The launcher requires an installed harness before the skill can run
(`rust/loopflow/src/lf/commands/run.rs`, `launch_prompt`). The skill's instruction
to stop and offer installation commands when no harness exists cannot bootstrap
its own execution. It also assumes a repository context. These are source
observations, not a live onboarding trial.

Jack clarified that init was an option to consider, that Loopflow prefers scoped
sessions, and that a repo-wide init might fit better: "if we can get by without
it, great". This Task therefore uses deterministic app actions for installation
and account connection. It does not launch an onboarding conversation, create
a synthetic Session scope, or require init after choosing a project. The
existing repo init remains available separately; redesigning it is out of scope.
Reuse installation and auth operations underneath the app rather than copying
the broad init skill into Swift. See `scratch/init-setup-review.md`.

### Download, drag into Applications, open

Jack confirmed this primary path during review. Keep the website's direct DMG
download. The Mac instructions show opening that download, dragging Loopflow
into Applications, and opening the installed app. No external Terminal command
or shell installer is required. The shell installer remains a supported CLI
route; it is not the fallback used to declare the Mac acceptance complete.

First launch must activate the copied bundle and its bundled `lf`/`lfd` through
the existing Rust installation owner before normal registry reads and sessions
start. Swift presents progress, a concrete failure and retry, and the provider
setup screen after activation settles. It invokes the bundled helper directly;
no preexisting PATH entry, daemon, coding tool, Git project, or install receipt
is assumed. A settled matching installation is reused on subsequent launches.

Add the minimum repo-independent bundle-activation entry to the existing
installer. Reuse preflight, retained artifacts, the promotion lock, migration
checks, machine entry registration, receipt, and recovery; do not implement
these decisions in Swift. Activation uses the bundle the person opened, not a
fresh download of whichever release is latest. Verify the signed bundle and
matching bundled artifacts through the existing validation owners. Preserve
release digest verification wherever downloading remains involved.

The coordinator must survive a GUI exit or replacement and reopen the activated
app when necessary. Existing promotion has quiesce/resume behavior, but its
first-install case with a running unregistered app is not yet proven. Resolve
that case in the same owner. Prove safe staging when the opened bundle is also
the target, interrupted activation and retry, and repeated opens without an
activation/restart loop. An old or incompatible bundle must not silently replace
a newer active installation or advance an unsupported store.

Use the actual copied app location as the installation target and record it for
updates. Support `~/Applications` when `/Applications` is unwritable, with a
Finder-based instruction in the install docs. Do not request sudo. If opened
directly on the mounted DMG, explain the drag step and allow retry after moving;
do not register the mounted image as the permanent installation. Reconcile
`install/published.rs` and the shell installer with the recorded destination so
updates do not create a second copy in `/Applications`. An explicit
`LF_APPLICATIONS_DIR` remains authoritative for the CLI route.

Update the opening of `docs/getting-started.md` and matching download-page
instructions. Put coding-tool and GitHub setup in the app section, with the
coding tool's separate cost beside any claim that Loopflow is free. Remove
`lf init` from the required newcomer sequence; keep its separate repository
setup documentation accurate. Leave the broader rewrite to LOO-306/309.
Retain accurate shell-installer completion output for CLI users: actual paths
and a correctly quoted optional `open` command, without launching a GUI from
headless installation. Linux and `--cli-only` never promise an installed app.

### Make terminal installation durable

After successful activation, the installation owner adds one clearly marked,
idempotent PATH block to the default Mac zsh's interactive startup file
(`${ZDOTDIR:-$HOME}/.zshrc`). The block conditionally prepends the actual install
directory. Preserve unrelated content, including a missing final newline;
quote paths with spaces or shell metacharacters as literals. Reinstallation
replaces the owned block rather than appending duplicates. Do not infer
persistent configuration from the installer's current PATH alone.

Share this operation between app activation and shell installation, with one
implementation in the existing Rust installer. The shell installer cannot alter
its parent shell. Its output and CLI instructions
must still include an immediate `export PATH=...` command for that shell.
Opening a new interactive zsh session must resolve `lf` automatically. For
other shells, retain explicit shell-appropriate/manual PATH instructions;
automatic configuration in this Task is limited to the default Mac zsh.
Report an unwritable startup file as installed-but-PATH-unconfigured with an
in-app retry and exact repair detail, without rolling back a valid app
installation. Normal successful DMG installation requires no manual shell work.

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

Once a tool is installed, the primary action is **Sign in**. Once the coding
tool is connected, establish GitHub sign-in through the existing auth owner.
If `gh` is absent, offer its installation as part of setup; if signed out,
offer **Connect GitHub**. Verify existing sign-in and skip completed steps.
Failed or canceled connection leaves an actionable retry. The mechanism for
acquiring `gh` is still to be specified; do not assume Homebrew is present.
Once both accounts are connected, the primary action is **Open project…**,
with a brief explanation that a project
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
| Does the download button run this installer? | No. `website/content.yaml` points at `Loopflow-latest.dmg`; the full verified installer is a separate path. The DMG bundles lf/lfd, but this research did not prove drag-install activation. | Jack chose to retain DMG acquisition; implement and prove activation from first app launch. |
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
| DMG-first app with first-launch activation | Requires activation, restart, registration, and recovery to work from a copied app. | Selected by Jack during review. Reuse the installation owner and prove this path on a clean account. |
| Existing full installer plus native app setup | Reuses the existing shell bootstrap but requires an external Terminal action. | Superseded by Jack's DMG choice. Retained as the separate CLI installation route. |

## Key decisions

- Use scoped app actions for setup. No init conversation or fabricated Session;
  repo-wide init remains a separate possibility outside this Task.
- GitHub sign-in is part of initial setup before **Open project…**, confirmed
  by Jack during review. Linear remains optional sharing.
- The finish is an open, connected app with **Open project…**, not a completed
  Task. This matches LOO-312 and does not borrow unfinished LOO-316 behavior.
- Download the DMG, drag into Applications, and open. Jack confirmed this
  entry path; first-launch activation through the existing installer is in scope.
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

- In scope: documented DMG-first Mac install route and first-launch activation;
  shared PATH persistence; writable app
  destination and update consistency; guided Codex/Claude installation and
  existing auth; early GitHub sign-in and supplying `gh` when absent;
  primary-window setup and project picker; missing-tool errors;
  focused docs/download changes and proofs.
- Out of scope: an init conversation or repo-init redesign; project creation,
  Git initialization, first local Task, Linear
  onboarding, GitHub delivery, Wave creation, flow visualizations, broad doctor
  dependency cleanup, general docs rewrite, pricing promises, and a second
  installation authority. No changes to the Wave's paused state or planning records.

## Done when

1. In an actual fresh macOS account, start with no lf, coding tool, gh, npm,
   Homebrew, repository, or saved app preferences. Follow the exact documented
   DMG download → drag → open route with no external Terminal command. Observe
   first-launch activation and any restart through to setup. Capture OS/account type, release
   or candidate identity, install destination, each action, and exact failures.
   A simulated HOME on a developer account does not satisfy this check.
2. Complete native installation and real browser sign-in for one supported
   provider and GitHub, including acquiring `gh` without a developer package
   manager. Loopflow then offers **Open project…**; the chooser works and
   cancellation preserves the action. Repeat the provider path for the other
   guided provider in a separate clean state before claiming both are supported.
3. Close/reopen the app and a Terminal window. State is re-observed, not remembered
   as a success boolean, and `lf --version` resolves. A repeat install produces
   no duplicate PATH block or activation/restart loop; an update retains the
   same app destination. Prove retry after interrupted activation and existing
   installation handling, including an older copied bundle. Test the
   `~/Applications` destination and launching from the mounted DMG separately.
4. Open an existing local fixture project and send a harmless prompt through
   the real app conversation launch. Observe a provider reply. No Linear
   connection is needed; GitHub sign-in was established during setup. Record
   CLT/Git availability separately: this
   follow-on proof must not introduce a hidden prerequisite for opening setup.
5. Focused automated proofs cover the owned shell block, escaped paths,
   unwritable profile, repeat install, CLI-only behavior, target selection,
   first-launch activation/registration/recovery, status failures,
   installation failure/retry, provider choice and config
   precedence, GitHub missing/signed-out/connected/failed observations and retry,
   same-window repository selection, and retained returning-user
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
- Counting shell-installer success as proof of the DMG path, requiring an
  initial external Terminal command, or registering a mounted DMG permanently.
- Writing the current Mac walkthrough from code before opening the running app.
- Changing only WavesView while leaving the primary no-repository view stranded.
- Calling a downloaded/installed binary authenticated, parsing auth prose, or
  making a failed readiness query look like a successful empty state.
- Adding another credential store, default-agent preference, repository list,
  installation authority, or onboarding-complete flag.
- Auto-installing a provider, signing in, or changing defaults merely on launch.
- Requiring npm, Homebrew, Rust, uv, Doppler, Linear, or a project just to
  open and set up Loopflow on the documented Mac path.
- Replacing user shell configuration, writing to the host from tests, requesting
  sudo, or bypassing release validation to make the demo pass.
- Presenting Open project as project creation or claiming a first Task is done.

## Internal slices

1. **Activate a dragged app.** Record the running baseline before UI edits.
   Trace and implement the copied-bundle entry into the existing promotion
   owner, including first-install app restart and durable helper registration.
   Isolate installer fixtures before profile writes; share PATH persistence
   across both install entries and retain the actual app destination on update.
   Prove the clean-account drag/open path and fresh-shell command discovery.
2. **Connect a coding tool and GitHub.** Add the small native installer operation and typed
   native-status projection in existing owners, reuse login, persist explicit
   default selection, and fix implicit no-tool diagnostics. Prove missing →
   installed → connected and failure/retry without a repo. Supply `gh` when
   absent and establish GitHub sign-in before offering **Open project…**.
3. **Carry setup through the primary window.** Reuse embedded shell and repo
   selection, implement setup actions and rechecks, preserve existing sessions.
   Prove the provider reply through a real local project.
4. **Close the public path.** Retain the DMG CTA and update narrow docs, perform
   the clean-account acceptance and public-channel trial when released. Keep
   the entire target above intact while implementing these internal cuts.

## This slice

Copied-app activation remains the current slice. The first implementation pass
repairs its existing installer process lifecycle: first-install target discovery,
shutdown limited to the selected paths, retained execution for bundled published
helpers, and verified reopening before or after store advancement. Helpers alone
do not request a window; a repeated quiesce preserves the original restart intent.

Still required: the repo-independent signed-bundle activation entry and Swift
first-launch integration, durable PATH setup, destination retention on updates,
and settled-registration/reopen proofs. The machine entry gate does not redirect
an unregistered bundled helper, so this entry can reach the existing installer
without inventing another authority. Rust currently validates helper identities
and tree digests; signature checking lives in the shell installer and must move
into the shared owner for bundle activation.

Record the live baseline before UI edits. This headless run supplies neither that
baseline nor the clean-account trial. No UI or newcomer walkthrough is changed.
The full design above remains the target; the process fixtures establish only
the repaired installer behavior, not a working download-to-setup journey.

## Slice ledger

| Evidence | Result | Limit |
|---|---|---|
| Initial branch inspection | Clean at supplied base; scratch contained only `.gitkeep`; repo guide and growth memory read. | No previous design to reconcile. |
| Minimal environment zsh probe | Login and interactive shells omitted `.local/bin`; explicit PATH resolved the fixture. | Developer OS, temporary HOME, stand-in executable. |
| Existing shell installer tests | 16 passed, 6.13 seconds. | Simulated release transport/promotion; no new-shell test. |
| Real installer script with isolated fixture transport | Exit 0; installed fixture; no profile files; new login shell could not find lf. | CLI-only simulation; app and machine activation stubbed. |
| Primary-window and auth source review | Corrected the assumed UI location and found reusable auth plus native-status gaps. | No rendered/live app observation. |
| Simulated design review | Added DMG-vs-installer distinction, safe test HOME isolation, same-window opening, and provider default persistence. | These are proposed changes, not implementation evidence. |
| Installer process lifecycle, 2026-09-26 | `cargo test -p loopflow --lib app_activation_tests`: 4 passed. Selected native fixture processes stop, another copy survives, helper-only shutdown requests no window, restart intent is persisted, and restart targets must match retained bytes. | Native `/bin/sleep` copies in temporary bundles; no real app, promotion, account, or browser sign-in. Initial fixture setup failed on a missing parent directory; fixed before the passing run. |
| Bundle staging, 2026-09-26 | `cargo test -p loopflow --lib artifact_tests`: 4 passed, including same-source/destination staging without changing the original bundle. | File fixtures only; not signed-release acceptance or interruption of a real promotion. |
| Implementation review, 2026-09-26 | Removed the app-ID-wide quit operation. A bundled published promotion helper re-execs retained bytes before shutdown so it is not one of the processes being stopped. No new receipt or installation authority. | Full helper re-exec, GUI-exit survival, registration, actual reopen, and repeated first launch still need configured-path proof. |
| Static checks, 2026-09-26 | `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` passed. | No affected-suite gate, Swift/UI tests, or public-channel trial in this implementation pass. |

## Measure

The chapter supplies no metric target. Record the number of manual PATH repairs,
package-manager prerequisites, and dead-end screens along each clean-account
trial. The acceptance target is zero for each, with one obvious next action at
the endpoint. Capture elapsed setup time excluding account purchase/creation as
descriptive evidence, without inventing a chapter KR or a speed promise.
