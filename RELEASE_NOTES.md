# v0.15.2

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.15.2 is about upgrading without losing what's running. In v0.15.1 an `lf install` could leave Desktop's bundled `lf` unable to open the store. A dev CLI did it during the release, and on November 4 a one-step destructive migration did it under 177 live `lf` processes, which cost those Codex conversations. In this release older builds keep opening a newer store. CI rejects migrations that would break them. Conversations keep running when the store or the `lf` executable goes away, and the CLI and Loopflow.app move together on every stream. Desktop also gets lighter and quicker: it no longer launches `lf` for every read, and conversations are already open by the time you click.

## Upgrades leave running work alone

v0.15.1 handled store skew by having each build refuse a store migrated past a "breaking" migration, with installers checking a reader catalog first. That kept the store readable but stranded processes: Machines got stuck on an old `lf`, old owners couldn't record anything, and status reads showed an empty registry. v0.15.2 removes that machinery and puts the rule in CI instead.

- **Older `lf` opens a newer store.** The reader floor is gone: no `-- breaks:` marker, no per-migration floor, no catalog check before installing, rolling back or switching streams. A build reads the ledger rows it knows, applies nothing past its own frontier and runs against the rest. (#1830)
- **CI catches destructive migrations.** `scripts/check_migrations.py` applies the branch's migrations over `origin/main`'s and compares the schemas. A new migration passes only if it is additive or a staged contract that drops what an earlier expand step superseded, after a waiting window. (#1827)
- **`lf install` says who will meet the new schema.** Before changing anything it lists the running processes still on older builds, and it keeps the pre-migration clone while any of them run. (#1827)
- **Skew is an error, not an empty board.** A store this `lf` can't open now fails the read, names the unknown migration and says to run `lf install`. Before, reads returned no Waves and no Tasks with exit 0, and agents took that as fact. (#1797)
- **The CLI and the app move together.** On a Mac every stream, `dev` included, promotes the CLI and Loopflow.app as one install. Green `main` now builds a Developer ID-signed, notarized Loopflow.app, and `lf install --dev` verifies it and links `~/.local/bin/lf` into it. Dev runs no in-app updater; `lf install --dev` and the schedule move it. Manual and scheduled updates may close and reopen Desktop. Running Session helpers and Graphs keep going, and a custom app location is kept. (#1777, #1815)
- **Sessions move onto the new build by themselves.** Owners, `lf resume` and rollover use one build comparison, and identical binaries at different paths count as the same build. An owner whose executable is removed or replaced finishes its active turn, ends while waiting for input, and its conversation reopens on the installed `lf`. Nobody has to kill processes. (#1799)
- **Stuck Sessions after a Desktop restart reconnect.** An owner from the earlier `lf` that can't write to the advanced store no longer leaves its Session showing "open in another terminal": opening it checks with the owner directly. An owner that runs but doesn't answer within a second reads as `detached`, offers Reconnect and continues the same conversation in a new owner. (#1788, #1790)
- **Pending work stops a takeover.** `lf resume`, rollover and automatic retirement share one rule. A recorded draft, pending request, live turn, running Graph or headless Run keeps its owner even when attaching fails, and returning doesn't replay input. (#1818)
- **Installation previews stay previews.** Validating a copied database no longer rewrites live preference files. The settings cutover runs at promotion. (#1824)
- **Saved account routes survive the cutover.** `account:` is exported as login emails, which is what launches select by, instead of account ids that failed with `account reference personal has no resident login`. (#1825)
- **One installed app.** `lf open` and `lf desktop` launch the installed Loopflow.app by path. Only release packaging uses the release bundle id, and promotion leaves one `Loopflow.app`, so Spotlight and Finder stop finding stray `.superseded` copies and benchmark bundles. Loopflow Dev now runs as the process `Loopflow Dev`. (#1798, #1792)
- **Every app build launches.** All five ways of building Loopflow.app embed Sparkle the same way and check that it loads before signing. Before, local, Dev, benchmark and relinked builds crashed in dyld on `@rpath/Sparkle.framework`. (#1794, #1796)

## lf records conversations; it doesn't carry them

A conversation belongs to the person and their provider. `lf` adds context and hooks and records what happens, and it shouldn't be able to break any of that.

- **Store failures become logged gaps.** When the store is locked, refused or gone, or the `lf` executable is reinstalled, hooks, typed input and quitting keep working. `lf` writes the gap to its log. (#1833)
- **Codex talks to its app-server directly.** The relay is gone. The launcher starts or resumes the thread, then execs `codex --remote <endpoint> resume <thread>`, and `lf` observes from the side. A thread started with `/new` or `/fork` now gets `lf`'s instructions and hooks too. (#1831)
- **Your harness settings win.** If no Loopflow setting names Chrome, Claude gets no flag and its own `claudeInChromeDefaultEnabled` decides. Before, every launch got `--no-chrome`. Codex threads aren't forced onto the standard service tier, and attended Codex Sessions don't get Loopflow's headless permission floor. (#1835)
- **Fewer processes per conversation.** An idle Claude conversation is one Loopflow owner and a thin viewer: the intermediate connection process and the launcher are replaced through `exec`. (#1802)
- **Idle Sessions end after two minutes.** `session.end_detached_after` drops from 24 hours to two minutes of quiet, measured in wall time so sleep counts. Drafts, pending requests, active turns and running Graphs keep their owner. Custom durations and `off` still work. (#1804)
- **Reconnects pick up where they left off.** Viewers resume from the last byte they received instead of starting from a snapshot, and the size and colors that apply can come from a viewer other than the one typing. (#1800)
- **Screens come back intact.** `libghostty-vt` replaces `vt100`/`vte`. It keeps primary and alternate screens, scrollback, hyperlinks, colors, cursor and Kitty keyboard modes. `lf resume` on an ended Session shows the last saved screen while the provider starts. (#1801)
- **Turn status stays accurate.** Native conversations no longer stay "Working" after a turn finishes, and a pending question or completed status survives a reconnect in Rex and Desktop. (#1809)
- **Background work reports in.** Open Task Sessions get quiet progress reports from their workers in Claude, Codex, OpenCode and Pi. Each run is tracked separately, so a newer helper can't hide an older worker's failure. Desktop shows workflow position, review readiness and expandable run history together. (#1806)

## Desktop: ready when you get there

- **In-process and idle-cheap.** Desktop calls Loopflow's Rust library instead of launching `lf` for every read, live update and action. Reads no longer record Process rows. Before, an idle Desktop could use a third of a core and the store collected about 100,000 unfinished `lf task diff` rows. Now an idle store costs each reader under 2% of a core. (#1773)
- **Conversations open before you click.** As a window loads, Desktop opens the selected Work first, then visible tiles, the repository and Waves, then recent started Tasks, two at a time. A load opens up to sixteen Tasks plus the repository and Wave conversations. `lf` now waits up to 30 s for Claude's trust-dialog lock. In a 23-conversation run all 23 answered; before, 14 were left at the dialog. (#1816, #1842)
- **Task shells live until Close.** A Desktop shell keeps running when you hide it, switch away or quit, and reconnects to the same execution locally or over SSH. A completed Task's worktree is removed about fifteen minutes after its last Session viewer leaves. (#1811)
- **One board rule.** `TaskSection` in Rust decides where every Task goes on every surface, so an unassigned Task leaves Working in the sidebar just as the Wave board shows it in Backlog. Sidebar drags land when you release, and abandoning a Task clears its row at once. (#1791)
- **Preferences live in files.** The CLI, Desktop and launch preparation use one settings catalogue, resolver and writer. Edits keep comments, hand edits are picked up, and both surfaces explain which value applies, including which agent each skill and Graph step uses. Private project and Wave preferences go in a local file in the main checkout. (#1805)

## Work moves into Tasks and terminals

- **`lf open` builds a workspace in your terminal app.** It opens a Task as a native tab in Rex, Herdr or cmux, with the conversation, a shell, files, changes and the Task's workflow in splits in its worktree. Every open is a fresh launch. Rex is detected from its environment or process ancestry; `--host rex`, `--server <endpoint>` and `--no-focus` steer it. (#1814)
- **Bring existing work into a Task.** `lf task create --session SESSION` wraps a Task around a live conversation, and `--worktree PATH` adopts a checkout. Files, history and unfinished input come along, and retrying with the same source returns the same Task. (#1807)
- **Waves are added and removed in one step.** `lf wave add [--objective …]` seeds a Wave's files and its first Project. `lf wave remove --preview` shows what would happen. Removing a Wave moves its open Tasks out and keeps history and unfinished file or provider updates for recovery. Desktop uses the same flow. (#1803)
- **Docs first in `pursue`.** A `write-docs` step writes the real user docs before `implement`, and implementation rereads them. `lf open --file PATH` shows files in an attached Desktop Files view. `pursue` also gains a QA pass. (#1817, #1834)
- **An `optimize` Graph** handles work that should move one measured number. (#1834)
- **Plainer builtins.** Builtin skills, surface prompts and `LOOPFLOW.md` are rewritten shorter and more concrete, about 1,450 lines fewer across 83 files, which every run used to pay for. The Loopflow skill passes a person's words to `lf` as data, not double-quoted shell, so `$(…)` or backticks in a request no longer expand. (#1834, #1795)

## Releases: regressions are GitHub issues

In v0.15.1 a `--regressed-in` mark held `stable` only on the Machine that recorded it. Now the record is a GitHub issue that every Machine reads.

- An issue labelled `regressed-in:vX.Y.Z` on the released repository holds `stable` below that release. Closing it as completed makes the first release published afterwards eligible once its week is up. `stable` is still GitHub's Latest marker. (#1780)
- `lf release issues`, `lf release regress` and `lf release promote` handle regressions and moving `stable`. `--regressed-in` is gone from `lf task create` and `lf task edit`. (#1780)
- If issues can't be read, the release still publishes, `stable` holds, and `lf release check` and `lf release promote` fail. (#1780)
- `lf release run` resumes after its release PR merges, without a manual fetch. Host releases use the installed `lf`, and the dedicated host follows `dev`. (#1780)

## Operational notes

- **Store compatibility.** The reader floor and `reader-catalog.txt` checks described in v0.15.1 are gone (#1830). Any build opens a store whose history extends its own, so rollbacks and stream switches don't wait. Destructive schema changes now go through expand and contract steps checked in CI (#1827). `tasks.regressed_in` stays in the schema with nothing acting on it, so released peers keep decoding it; a later Task (LOO-730) drops it.
- **Settings cutover.** Preference overrides move from the database into files at the first promotion of this build (#1805, #1824). Account routes are exported as login emails (#1825). Check `docs/config.md` if a route stops resolving.
- **`LF_INSTALL_CLI_ONLY=1` / `install.sh --cli-only`** now only skips first-time Desktop setup. An existing app is updated with its CLI. (#1815)
- **Scheduled updates.** `docs/ops.md` now covers changing the cadence, reading `~/.lf/logs/refresh.log`, and turning `com.loopflow.refresh` off and on. Every install registers the job, and on Linux the next install restores the crontab line. Help and docs no longer say rollover waits for viewers or that retained builds are a way back. (#1826)
- **Update scripts.** Remove `--regressed-in` from `lf task create|edit` calls and use `lf release regress` (#1780). Scripts that assumed Claude got `--no-chrome` by default should set `chrome: false` (#1835). Expect detached Sessions to end after two minutes unless `session.end_detached_after` is set (#1804).
- **Linear.** Task creations reach Linear again. A freshness check inside every write had used up each creation's five-second deadline, and the retry loop sent about 4,000 requests an hour. Creations now send directly, an idle pass over pending exports costs nothing, and `lf task sync ISSUE` finds a stuck creation. (#1829)
- **Disk cleanup is opt-in.** An explicitly installed hourly launchd runner replaces the cron agent; installing Loopflow never turns it on. It targets ignored, untracked build output across repositories and worktrees, keeps source, caches, Trash and history, and records what it removed. It runs at launchd's Standard policy and gives `ps`/`lsof` the two-minute command deadline. Docker images with no creation time are kept. (#1820, #1821, #1823)
- **Dev packaging works again.** From #1811 on, `Packages (main)` failed on every main commit, so `lf install --dev` stayed on e3c888924 (#1791). The release resource check launches the signed app with `-ui-test-mode empty-workspaces`, and the app wrote its snapshot and then never exited. Quitting from inside a SwiftUI task left AppKit's terminate loop waiting on a reply that could only run once that loop ended. Quit now detaches relays right away and exits at once when no relay is running. ⌘Q was never affected. With main packaging again, `lf install --dev` and the scheduled update can move past that commit. (#1846)
- **Demos and gate under load.** The `demo` skill keeps history in a durable root apart from a disposable runtime root, and stale demo owners retire. Gate runs stop adding work while macOS security daemons are saturated. In one case, stopping six stale demo owners brought `syspolicyd` down after restarting the service alone had not. (#1822)

## Small changes

- `lf account <provider> use --help` and `lf help account codex use` resolve the way the command runs. Stale command names in comments and the hotfix skill are fixed. (#1721)
- Deleting a stack's lower branch retargets the open PRs above it to the default branch first, so GitHub doesn't close them and `lf wt prune --branches` doesn't delete their branches. (#1789)
- GitHub shows the product README; the CI notes moved to `.github/CI.md`. (#1793)
- Agent OS processes have their own `AgentProcessId` type, so mixing them up with `lf` process IDs no longer compiles. There's no migration. (#1840)
- The loopback-SSH viewer acceptance test ignores zombie processes, so a failure there means a real extra PTY. (#1841)