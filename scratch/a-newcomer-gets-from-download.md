# From install to an open Loopflow

Status: accepted direction; Rust installer activation, PATH, destination,
native agent/gh install and provider/GitHub status and sign-in implemented
(see This slice). Swift setup and live Mac acceptance remain outstanding.
Baseline: synced onto main `98dab5ea8e` (v0.14.15).
Research performed 2026-09-26 (run-local date).

## Problem

Kim or Scott should be able to install Loopflow on a Mac without a coding tool,
open it, connect a coding tool and GitHub, and see one useful next action. The current
instructions assume a working shell PATH, an authenticated coding tool, and a
Git project. The primary window offers no obvious way to acquire those first
two things or open the third.

Jack's accepted direction in `.lf/wave/growth/MEMORY.md` makes the Mac app the
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
delivery. Reuse `lf account connect github` and supply its `gh` executable
on a clean Mac (`lf install gh`). Verify and reuse an existing sign-in. Authentication
does not authorize creating repositories or publishing code automatically.

Jack chose "drag to app -> open" as the primary acquisition path. Jack
prefers scoped sessions and is content to avoid an init conversation here.
Initial setup uses app actions; a repo-wide init is a separate possibility,
not part of this Task. `lf install gh` resolves the no-package-manager `gh`
constraint (choice in `questions.md`). The account, init and DMG review notes
are in git at `8278b4a210` (`scratch/*-review.md`).

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
the broad init skill into Swift.

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
(`.zshrc` in ZDOTDIR, else HOME). The block conditionally prepends the actual install
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
Failed or canceled connection leaves an actionable retry. `gh` comes from
`lf install gh` (GitHub's release archive); Homebrew is never assumed.
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

`lf install agent <codex|claude>` runs the provider's official installer only
on explicit request, from a downloaded file, and verifies the executable
uncached; provider updates stay provider-owned and no Loopflow promotion or Git
is involved. `GUIDED_AGENTS` is the single definition; Swift duplicates no
download command or OAuth.

Status, sign-in and the default-agent choice are implemented in the existing
`lf account` owners (see This slice): a typed native reading, never parsed
prose or a cached token; sign-in through the provider's own login; and
`--default-agent` writing Home `agent` only after verified success, so
repository/skill/explicit precedence is unchanged and installing alone never
changes the default. No second credential store or Swift preference.

For an implicit launch with no available coding tool, replace the misleading
Codex-only error with both supported setup choices and a link to the setup
instructions. Explicit `-m codex` failures still name Codex. Preserve the current
installed-tool ordering when no explicit setup choice or configuration exists.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Does the shell wall exist? | The real installer, with isolated HOME and stubbed transport/promotion, exited 0 and put a fixture in `.local/bin`. No startup files were written; a new `zsh -lic` reported `lf: not found`. A separate minimal PATH probe failed in both login and interactive zsh; explicit PATH succeeded. | Fix persistence and distinguish the current shell from future shells. This is shell simulation, not clean-account proof. |
| Does the app depend on terminal PATH to find lf? | `LocalWaveAgentLauncher.controlLfPath` resolves the bundled helper; `GUIProcessEnvironment` already adds `.local/bin` and common package-manager directories. | Reuse these paths. Do not replace bundled-helper authority with PATH fallback. |
| Does nothing support sign-in? | `lf account connect` (then `lf auth`) already invokes Claude/Codex native login, opens the browser and waits for completion. | Expose the existing path. No second OAuth client or credential store. |
| Is npm required? | Both official provider docs now offer native macOS installers; see sources below. | Install without Node, npm or Homebrew as prerequisites. |
| What happens with no tools? | `default_agent()` falls back to Codex; `missing_agent_message` then gives a Codex/npm-only instruction. Availability checks are cached. | Distinguish implicit absence from an explicit provider request; re-probe after install. |
| Is the claimed empty screen current? | Code: primary launch is `PodiumView` → `WorkspaceNavigator` when no repo is selected. Its menu can contain no repository and no actions. “Author a Wave” belongs to `WorkSurfaceView`; `WavesView` is a different scene. | Change the actual primary boundary. Live confirmation remains required before UI implementation. |
| Does the download button run this installer? | No. `website/content.yaml` points at `Loopflow-latest.dmg`; the full verified installer is a separate path. The DMG bundles lf/lfd, but this research did not prove drag-install activation. | Jack chose to retain DMG acquisition; implement and prove activation from first app launch. |
| Can a standard account use the current app installer? | The shell installer exits if `/Applications` is unwritable. An explicit target already exists. | Select a user-writable target without sudo and keep later updates at that target. |
| Can setup work with no repo? | Conversation launch requires repo scope (source read). Native terminal surfaces and provider-auth commands need no project concept. | Give setup a shell surface, not a fabricated conversation/Task. |
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

1. **Activate a dragged app.** (Rust side implemented; Swift entry and
   live proof outstanding.) Record the running baseline before UI edits.
   Trace and implement the copied-bundle entry into the existing promotion
   owner, including first-install app restart and durable helper registration.
   Isolate installer fixtures before profile writes; share PATH persistence
   across both install entries and retain the actual app destination on update.
   Prove the clean-account drag/open path and fresh-shell command discovery.
2. **Connect a coding tool and GitHub.** (Rust side implemented; live
   proof outstanding.) Add the small native installer operation and typed
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

Status (2026-10-22): copied-app activation's Rust side and the first provider
setup action are implemented; Swift first-launch integration, the remaining
setup actions and all live acceptance are outstanding.

Implemented in the existing installer owner:

- `lf install activate-app --bundle <app>` runs from the opened bundle's own
  helper (digest-checked, also after promotion re-execs retained bytes). It
  refuses disk-image/read-only-volume and AppTranslocation copies with the drag
  instruction, and a copy older than the active release. It then runs ordinary
  published promotion with the bundle as source and target, then writes PATH.
- Published promotion with an app source verifies `codesign --deep --strict`
  and `spctl --assess` before any target changes; `release/install.sh` no
  longer verifies signatures itself.
- `ActiveInstall.activation` records the activation targets (absent in records
  written by released lf; those read as `None`). `app_destination()`:
  `LF_APPLICATIONS_DIR` > recorded app > `/Applications` if writable >
  `~/Applications`. Promotion resolves it when `--app-target` is omitted;
  `install.sh` and `scripts/install.py local` never pass a target, and
  `lf install`'s current-release check uses it too.
- `lf install shell-path` and activation share one marked block in
  `.zshrc` in ZDOTDIR, else HOME (symlink-followed, idempotent, single-quoted
  path, failure names the exact `export` repair). `install.sh` calls it on
  macOS and prints a quoted `export` for the current terminal. Installer
  pytest fixtures always set HOME and ZDOTDIR.
- `lf install agent <codex|claude>` downloads the official installer to a
  file, runs it from home and succeeds only when the executable answers
  `--version` (uncached). `GUIDED_AGENTS` in `engine/agent.rs` is the single
  definition; missing-tool errors and doctor fallbacks name these installers
  and offer both tools.

Provider/GitHub setup actions (2026-10-22, second pass). The command is
`lf account` on the base (renamed upstream); the design's
`lf auth status <provider> --json` is `lf account [provider] --json`:

- Local rows for codex, claude and github carry a fresh native reading:
  `executable` (`installed|missing`, PATH as a launch sees it; null for
  managed rows, other providers and `--cached`), `verification` accepted /
  rejected / unavailable (failed or 20 s timeout, never signed out), the CLI's
  login, and `recovery` (`lf install agent <tool>`, `lf install gh`,
  `lf account connect <provider>`). `auth_status.json` pins it; Rust
  round-trips it and Swift decodes `ProviderAccountReport.setupState`.
- Ambient Codex without `auth.json` asks `codex login status` (Keychain
  logins). Ambient Claude no longer sets `CLAUDE_CONFIG_DIR`, which moved its
  login to a home-scoped Keychain item plain `claude` never reads.
- `lf account connect <provider>` for codex/claude/github with no remembered
  Chrome profile signs in through the default browser, then requires the CLI
  to report the login. `--default-agent` (codex/claude) writes Home `agent`
  only after that reading, skipping sign-in when already connected; malformed
  config is reported and left as is (shared `config::update_global_config_at`,
  also used by portfolio).
- `lf install gh` installs GitHub's release archive for this machine into
  `~/.local/bin` after checking the release's SHA-256 list; an existing `gh`
  on PATH is left alone.

Remaining:

1. Swift first-launch call to activate-app (progress, failure, retry; app is
   quiesced/reopened by promotion when source == target — unproven live).
2. Primary-window setup view reading `lf account --json` and running the
   recovery commands in its shell, only after the running-app baseline is
   recorded.
3. Docs, then the clean-account Done-when trials (incl. real installer,
   browser sign-in and `gh` download, none exercised here).

The live baseline and clean-account acceptance are open gaps; Jack directed
that fixtures not substitute for them. The retained-helper
GUI-exit survival and real promotion/recovery entry remain unproven
(prior reviews, commit `d5c043c13a`).

## Slice ledger

Earlier lifecycle, completion and compression reviews (2026-09-26) are in git
at `d5c043c13a` (`scratch/installer-*-review.md`, `compress-review.md`); their
open findings are folded into Remaining above.

| Evidence | Result | Limit |
|---|---|---|
| Shell/no-PATH probes (kickoff) | Fresh zsh lacked `.local/bin`; installer wrote no startup file. | Temporary HOME, developer OS. |
| Lifecycle + completion fixtures (2026-09-26) | `app_activation_tests` 7 passed; `machine_install::tests` 23 passed; fmt/clippy passed. | Native sleep stand-ins; no real promotion, keeper or GUI. |
| Installer/agent slice (2026-10-22) | `cargo test -p loopflow --lib` for install::{shell_path,bundle,published,agent}, machine_install, app_activation_tests, engine::agent: all passed (38 + 64 + 16 in focused runs). Fresh `zsh -ic` resolves a path with spaces/quotes/`$`/backquotes. Built `lf install shell-path` with `env -i` HOME/ZDOTDIR twice: one block, `command -v lf` resolves. `activate-app` on a non-bundle and on a mismatched helper refuses before any state read. `test_shell_installer.py` 19 passed. clippy/fmt/ruff clean. | No real signed bundle, promotion, provider installer download, or clean account. Affected-suite gate not run. |
| After main sync + recovery fix `257ed79113` (2026-10-22) | `cargo test -p loopflow --lib -- lf::commands::install machine_install app_activation_tests`: 37 passed, 2 ignored. | Same fixture limits. |
| Setup actions (2026-10-22) | `cargo test --lib -- lf::commands::account provider_auth`: 120 passed; integration `auth_tests` 11 passed (missing→installed→connected without a repo; default agent written only on success; malformed config kept); `install::gh` 2 passed; Swift `DTOFixtureTests` 50 passed; clippy/fmt clean; CLI reference regenerated. | Stub CLIs; no real provider, browser, Keychain or GitHub download. |
| Compress (2026-10-22) | One local `AccountRow` path, `connect` split into sign-in/verify, config write inlined in `update_global_config_at`; clippy clean; account_status/portfolio lib tests and 3 integration auth tests passed. Unrelated `session_record::manifest_round_trips…` fails on missing `graph_process_steps` (untouched by branch). | Same stub limits. |
| Realign (2026-10-22) | Plan matched code; origin/main unchanged since `98dab5ea8e`; no tests rerun. | Reuses results above. |

Proposed Wave memory update after landing (memory is read-only here): items 1,
3 and 7 of "What stands between a newcomer and a first task" are addressed in
code on this branch for zsh and Codex/Claude; clean-Mac proof still absent.

## Measure

The chapter supplies no metric target. Record the number of manual PATH repairs,
package-manager prerequisites, and dead-end screens along each clean-account
trial. The acceptance target is zero for each, with one obvious next action at
the endpoint. Capture elapsed setup time excluding account purchase/creation as
descriptive evidence, without inventing a chapter KR or a speed promise.
