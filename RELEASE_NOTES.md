# v0.16.0

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.16.0 makes work easier to return to—and cheaper to keep running. Conversations survive more installation, server and connection failures; Tasks can move between Machines, continue on another configured agent and retry a failed step without repeating completed work. Desktop, terminal and iPhone gain more direct paths into the same work, while shared observation and narrower database reads reduce the overhead around it. These notes cover the cycle since v0.15.0, including changes already delivered in v0.15.x patches. (#1812, #1854, #1887, #1899, #1905, #1908, #1928)

## Return to the conversation

The cycle began by repairing attachment after upgrades and Desktop restarts. It ends with fewer per-Session owner processes: bare terminal launches run their agent directly, managed conversations use holders beside the server, and headless Work runs as ordinary background processes. (#1788, #1799, #1887, #1895, #1904)

- **Bare terminal launches belong to the terminal.** Both bare `lf` and commands such as `lf implement` or `lf -a codex debug` run without a per-Session owner between the terminal and agent. This replaces the earlier approach based on a two-minute unviewed timer, a native-input ledger and a sixteen-conversation cap. Task conversations end when their Task closes. (#1877, #1895)
- **Managed conversations survive installation and server restarts.** Task and Wave agents run in a holder beside the server, keeping turns running through `lf install` or a server crash. Desktop shells share holders with conversations. (#1887, #1904)
- **Loopflow recording failures need not break the provider conversation.** Hooks and recording become best-effort, and Codex clients connect directly to their private app-server while Loopflow observes beside them. Turn hooks use the Machine server; Pi and OpenCode plugins also send events and collect steers directly instead of repeatedly starting `lf`. (#1831, #1833, #1879, #1889)
- **Reconnect to the execution already running.** Local terminals recover connections without counting laptop sleep against heartbeat deadlines. Session viewers can resume from their last received byte, restore more terminal state and show a saved screen while reopening. Queued keystrokes already sent before viewer closure are delivered in order while that viewer still holds control. (#1800, #1801, #1839, #1876)
- **Opening respects other views.** Exact Session links reach the saved conversation, including taskless Sessions. `lf open` can open one conversation where requested, report an existing view without taking its input and retry a failed attachment; Desktop can refresh stale panes. (#1863, #1882)
- **An agent switch stays switched.** Provider handoffs keep the Session open, and subsequent resume, Desktop and phone opens retain the selected agent instead of reverting to the original provider or passing along the wrong provider's conversation ID. Terminal titles follow Session renames and reattachment. (#1857, #1900)
- **Finished runs leave the open list.** Headless Sessions move to history when their processes exit, retaining their conversations, usage and activity. Session lists and cards show current or last turns for Claude, Codex, Pi and OpenCode, including conversations started outside Loopflow, with model and usage where available. (#1880, #1893)

## Start, direct and move the work

Starting a Task, joining its conversation and following its progress now fit together. Dependent work can start earlier, and moving work between Machines carries a continuation rather than merely leaving instructions to start again. (#1761, #1806, #1888, #1894, #1899)

- **Create work from a request or bring existing work along.** New Task connects a request to its running conversation. Existing checkout work and a live conversation can also become a Task without losing files, history or unfinished input. (#1761, #1807)
- **Open as the run starts.** `lf task run <task> --open` opens the Task's primary conversation as launch returns. Task launch also prepares its conversation so resume has something to reattach to, and open Task Sessions receive background worker progress. (#1806, #1844, #1894)
- **Use one verb for direction.** `lf steer TARGET 'message'` addresses a Session or Task; `lf --wave W steer` and `lf --repo . steer` address a Wave or repository. Every launch, including bare headless children, reads its own steers at hooks, and Task updates use the same delivery mechanism. (#1923)
- **Retry the failed step.** Automatic retries and the next `lf task run` resume at the failed step, so a landing failure does not repeat completed gate work. Standalone Graphs such as `lf incident` can run in a completed Task checkout without changing that Task's Graph position or completion. (#1919, #1928)
- **Move a Task to another Machine and continue there.** `lf task move --machine mini` carries the Graph continuation and starts it at the destination. Moves retain progress and conversation state so the same command can recover an interrupted transfer, and moving back works too. Status distinguishes execution location from transfer progress. (#1899, #1921)
- **Stack before the parent's PR exists.** A child can start and publish from a placed parent branch. After the parent lands, Loopflow repairs a stale child base and retargets upper PRs before deleting lower branches, avoiding accidental closure of the remaining stack. (#1770, #1789, #1888)
- **Stopping shipping holds.** `lf ship --stop` remains in force until the next `lf ship`. Detached Task launch output says when a run may merge its PR; repeated identical Auto-Ship failures stop looping, and terminal stops withdraw Auto-Ship approval. (#1770, #1775, #1866, #1908)

## Continue when capacity changes

Account exhaustion and unavailable models now have recovery paths, with later fixes making fallback work even when an account refuses before any provider starts. The final behavior follows configured agent choices and stops exhausted runs once rather than repeatedly restarting them. (#1828, #1906, #1908, #1935)

- **Graphs can continue within the current step.** Recovery can use another eligible account, model or configured provider. Recognized model-unavailable refusals also advance to the next configured agent. Exhausted runs stop with recovery advice and can restart at the refused step. (#1828, #1906, #1908)
- **Interactive blocked Sessions can move together.** `lf session move --blocked` and Desktop's **Move Blocked Sessions…** action provide a recovery path for open conversations. Automatic interactive quota handoff is not included. (#1828; previous v0.15.3 notes)
- **Claude usage refreshes release stale restrictions.** Refreshing recovered capacity replaces matching old model-limit records. Exhausted Fable quota leaves other models available, account reports show which models each login can launch, and clearing cooldowns releases stale restrictions. (#1937, #1939)
- **Launch preferences are easier to inspect.** CLI and Desktop edit the same file-backed preferences and explain which agent each step uses. Loopflow leaves harness settings in control when it has no override; explicit Codex standard or fast service-tier settings are available globally, per repository or per invocation. Resume also works with agent-list preferences. (#1805, #1835, #1870, #1919)

## Reach the same work from more places

Desktop gains a shared local server and recovery for uncertain results. The iPhone app and terminal workspaces extend access to existing work rather than requiring another conversation to be created. (#1812, #1813, #1814, #1854)

- **Continue from iPhone.** The native app can reply to existing Sessions, review changed files and create Tasks on a Machine over Tailscale. Mobile terminal attachment uses single-use tickets and requires the phone to be signed in as the host owner's Tailscale user through `lf server mobile`. (#1812, #1897)
- **Open a full terminal workspace when needed.** Task workspaces open as fresh native tabs with splits in Rex, Herdr and cmux, bringing the conversation, shell, files, changes and workflow together. Task shells survive hiding, quitting and reconnecting. (#1811, #1814)
- **Desktop shares the CLI's operations through a local server.** This supersedes the intermediate in-process Rust bridge. Terminal traffic uses separate connections, and ordinary CLI commands remain usable without the server. (#1773, #1813; previous v0.15.3 notes)
- **Lost creation replies can be recovered.** Desktop retains submitted Task and Session creations across reconnects and app restarts, recovers the original result and keeps uncertain launches inspectable without starting them again. Late New Task replies respect later navigation and newer drafts. (#1854, #1872)
- **Live views follow the facts they display.** Task subscriptions follow file edits, process exits, PR and CI changes. Reconnectable watches avoid resending unchanged readings, and Session status reporting no longer requires a process per pane. Idle panes no longer poll connection status every 100 ms. (#1853, #1865, #1871, #1875)
- **The board and sidebar agree.** Unassigned Tasks leave Working, shipping Tasks remain visible in Autonomous, counts exclude finished work, drag placement completes on release and abandoning clears the row immediately. Waves can be added, renamed or removed while retaining unfinished work and history. (#1762, #1791, #1803)

## Keep busy Machines responsive

Many conversations share one SQLite write lock. This cycle reduces the work competing for it: identical watches share readers, large-history queries use narrower indexed reads, and planning preparation moves outside write transactions. These changes reduce repeated work; they do not establish a total CPU or throughput guarantee. (#1868, #1905, #1909, #1910, #1918, #1930)

- Work views batch Task reads, Graph history records Task identity for indexed lookup, and Session history reads narrow to the requested Task, time window or input. Foreign-key child lookups gain indexes, and query-plan checks catch large partial-index scans. (#1907, #1910, #1916, #1918)
- Planning refreshes avoid rebuilding unchanged views and unnecessary write locks. Peer sync prepares history merges before taking the write lock, and Waves without a Linear Initiative can show local planning. (#1901, #1930)
- Linear traffic backs off as its budget shrinks, with visibility into commands spending it. Task creation no longer burns its deadline on a repeated freshness check, and sync accepts later field readings at the same revision instead of leaving edits or completions stuck. (#1766, #1829, #1898)
- Opt-in hourly cleanup targets disposable coding output while retaining source and history. Agent helpers no longer hold idle build output indefinitely; Docker images with unknown age are retained. Worktree pruning explains kept checkouts, and finished Task worktrees can clean themselves up. (#1774, #1811, #1820, #1821, #1891)

## Operational notes

The release cycle changes both installation policy and scripting contracts. Upgrade Desktop and its CLI/server together, and update automation using removed command paths. (#1776, #1813, #1815)

- **Choose an installation stream.** `lf install --stable`, `--preview` and `--dev` select the Machine's stream. Releases first publish as preview; stable promotion waits a week and can be held by GitHub regression issues. Mac dev installs include a signed app built from green main, and Desktop and CLI update as one installation. (#1772, #1776, #1777, #1780, #1815)
- **Older readers are no longer refused by a reader floor.** The runtime reader-floor check is removed. CI instead checks staged destructive migrations, and installation keeps its restore point while older builds still run. Migration ordering follows main's merge history and is retained in published source archives. (#1755, #1827, #1830, #1884)
- **Command namespaces changed during the cycle.** The `lf ops` parent is removed. Commit, sync, account, PR, worktree and Machine commands live directly under `lf`; repository CI, people, tokens and release operations live under `lf repo`, with planning configuration at `lf repo plan`. Settings use `lf config settings/get/set/unset`. (#1813; previous v0.15.3 notes)
- **Creation JSON and recovery are part of the API.** Task creation returns `{task, graph, session_id, milestone}`; read the identifier at `.task.identifier`. Task and Session creation accept `--request-id`, and `lf server receipt REQUEST_ID --json` inspects the result. An `outcome_unknown` launch response leaves work running: inspect status before repeating it. (#1854, #1859; previous v0.15.3 notes)
- **Provider login routing changed.** Each Machine uses one home per provider. The old isolation setting remains readable but is ignored; `--isolate`, `--shared`, `lf account route` and `lf account token` are removed. (#1828; previous v0.15.3 notes)
- **Release automation is more recoverable.** Minor releases can cut from current main with prepare hooks, and release runs can continue after their PR merges. Scheduled macOS jobs run where they can reach the Keychain. Packaging preserves Sparkle framework signatures, source packages build without the repository checkout, and notarization failures expose Apple's explanation with a daily account-access check. Intel Mac CLI packaging cross-compiles on Apple Silicon. (#1763, #1764, #1780, #1786, #1881, #1927, #1936)
- **Recovery still needs installed-product verification.** The supplied v0.15.3 notes distinguish simulated relay disconnection from physical laptop sleep and leave native input and remote recovery acceptance incomplete. The included later sources do not establish that those checks are complete. (#1858, #1863, #1876; previous v0.15.3 notes)

## Small changes

- `feature` begins with explore; `pursue` plans with kickoff, reviews concepts before refresh and compresses immediately before QA. An `optimize` Graph is added. Implementation starts with user docs that Desktop can present for editing. (#1817, #1834, #1903, #1915)
- Gate publishes a candidate before reviewing so hosted CI starts sooner. Test guidance requires deterministic fixes or deletion for flaky tests, rather than reruns; focused transport and core-decision fixtures reduce full-Machine setup, and checks catch store paths that take a write lock without changing rows. (#1917, #1922, #1924, #1925, #1926)
- Failed macOS runs on main report the tested commit and unchanged `last-green` instead of opening automatic revert PRs. Desktop launch and quit hangs are checked before merge. (#1850, #1932)
- `lf doctor` reports how Desktop last exited. Finder, Spotlight and `lf open` target the installed app, while Loopflow Dev has its own process name. (#1771, #1792, #1798)
- Typed board, Task-detail and Session-card reads join subscription events for shipped Tasks, steers and finished Graph steps. Agents receive their Session and Task identity in start and compaction context. (#1869, #1873, #1885)
- PR writing guidance drops repeated test and lint listings, builtin instructions use plainer language, and repository guidance calls for fixing clear code problems encountered during a Task. Ruff advances to 0.17.4. (#1782, #1834, #1892, #1902, #1911)