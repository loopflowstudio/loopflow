# v0.14.11

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.11 makes Loopflow work easier to reach, and it causes less damage when two things run at once. Every Task, Wave and repository now has a `loopflow://` address. `lf open` takes you to that conversation wherever you are working: cmux, Herdr, Desktop or a plain terminal. Underneath, an older `lf` can keep using a store that a newer release only extended. Concurrent writers wait for each other instead of failing, and `lf task run` keeps the terminal free by running each edge's Flow in the background. Upgrade if you switch between hosts, run more than one `lf` install, or run several Flows at once.

## One address for every conversation

Before this release, getting to a Task's conversation took `lf desktop open`. That worked only with the Mac app and only from inside the right checkout. An address now names the work, and `lf open` takes the address to its conversation from wherever you are. (#1601, LOO-426)

- `lf open loopflow://<repo>/task/<id>` also accepts `/wave/<name>` or a bare `loopflow://<repo>`, and it replaces `lf desktop open`. An address names the repository by folder name. It never carries a path, Session or host. If several registered repositories share a folder name, add `?repository=<id>`. Without it, nothing opens and the qualified links are listed.
- `--host auto` tries the Herdr pane or cmux workspace that ran the command, then Desktop (macOS, when installed), then a plain terminal. Hosts are detected only from the caller's own environment. Desktop now sets `LF_DESKTOP` on its children and drops any inherited cmux and Herdr variables. A clicked link has no caller, so it opens in Desktop. `--host desktop` reaches Desktop from a cmux shell. If a destination refuses, the next one is tried. If a request goes unanswered, that is reported and the request is never repeated elsewhere.
- Terminal destinations go through `lf session connect`, so the existing ownership and attachment rules decide what opens. A conversation that is already live in your host gets focus, with its draft intact. One that is live in another client is reported as `elsewhere`, along with its `--takeover` command. A private `LF_HOME` now carries over into the host workspace.
- `lf open --available` opens every conversation that is available for you to work with, across registered repositories. `--repo`, `--task` or `--wave` narrows the set. `--link` prints a readable link without opening anything: OSC 8 on a terminal, plain text when piped, or `--format markdown`. `--json` adds the route and a copyable `lf open` fallback.
- `lf install` registers a `loopflow:` link receiver (a small macOS app or a Linux desktop entry) and keeps any handler you had already chosen. It also installs a Herdr link handler, so Control-click opens the conversation beside the clicked pane.
- Desktop now resolves addresses through `lf open --link --json` instead of parsing them in Swift. Agents open Work themselves when that is clearly the next step. Otherwise they print a `[LOO-489 · title](loopflow://…)` label.
- "Waiting" is now "available" across the product. `--waiting` becomes `--available`, and themes gain a separate *available* colour.

## Mixed versions and concurrent writers share a store

Before #1608, once a newer `lf` migrated the store, every older `lf` was locked out, even when the newer release had only added a nullable column. Running two installs side by side, or rolling back after a promote, meant upgrading everything or deleting the store. Each migration now records a **reader floor**: the oldest `lf` that can still open the store. (#1608, LOO-521)

- On open, `lf` accepts migrations newer than it knows about when their highest floor is at or below its own version and every object it expects is still present and unchanged. Otherwise it refuses with `this store needs lf <floor> or newer …; install it with lf install`.
- The release cut proves additivity by replaying released history. A batch made only of new tables, non-unique indexes and nullable or defaulted columns keeps the previous floor. Any other change raises the floor to the release being cut. Draft authors can force a break with `-- breaks: <reason>`. Nothing can mark a draft additive.
- `lf install promote` prints the floor that pending migrations would raise. A rollback can now select an older `lf` that the floor still admits, without applying anything.
- Older writers leave new columns `NULL`, and persisted JSON no longer rejects unknown fields.

Store writes also stop failing under contention. Write connections now begin transactions as `IMMEDIATE`. A transaction that reads and then writes therefore waits for another process's commit under the busy timeout, instead of failing with `SQLITE_BUSY_SNAPSHOT`. Recording a step Process or a Workflow arrival while a sibling Flow writes was the common case of this failure. (#1618)

## Task runs stay out of your way

You take part in a Task at its Workflow nodes, in the Task conversation, so the Flows that run its edges no longer take over the terminal.

- `lf task run` starts the edge's child `lf flow` with `--batch` unless you pass `-i` or `-b`. Use `lf -i task run …` to keep an edge in the terminal. (#1618)
- Headless steps no longer print the `/status, /interrupt, /detach` controls banner into their run output. The banner goes only to a viewer that can type, when that viewer attaches. Observers and the captured output tail never see it. `/detach` now drops only attached viewers and leaves the execution's own control connections in place, so the next viewer still sees the controls. (#1623, LOO-545)
- The builtin `pursue` Flow now runs `concept-review` headless after implementation: `implement → compress → refresh → loop-or-next → concept-review → loop-or-next → pr-publish`. If the review changes the design, pursue goes back to implementation. If it finds an open product choice, pursue stops Blocked and names the question. `pursue-auto` gets the same pair before `gate`. Each pursue now runs one more agent step. (#1611, LOO-525)

## Branch cleanup with evidence

`lf wt prune --branches` (with `--dry-run`) deletes the origin and local copies of a branch only when it has evidence of finished work at the branch's current head. That evidence is a merged or closed PR, a finished Task whose work is already in the default branch, or a `release-candidate/…` branch whose tag exists at the commit in its name. An open PR, a worktree or an unfinished Task always keeps the branch. Every decision is printed with its reason, followed by totals and the remaining GraphQL budget. (#1616, LOO-539)

- Remote deletes use `--force-with-lease` at the head that was observed. A branch that moved in the meantime is kept. One that another machine already deleted is reported as already gone.
- GitHub failures are named: `Auth`, `RateLimited`, `TimedOut` or `Unavailable`. Missing GitHub facts stop the whole run before anything is deleted, and so does a GraphQL budget below 200 points. Both exit 1.
- When `lf land` merges locally, it now deletes the remote branch through the same lease-checked path, so a remote that has received new commits is left alone.
- Limits: this machine cannot see worktrees on another machine. Branches from the retired release code are kept as "no finished-work evidence."

## Contributor tooling: preview copies and fake worlds

These changes affect only people who build `lf` from source. Released builds carry no drafts, so they never use a copy.

- **Draft builds run on a copy of the real store.** A build that carries unreleased migration drafts no longer touches `~/.lf/loopflow.db`. It snapshots the store into `~/.lf/copies/<draft digest>/`, applies its drafts there and says so on stderr. Copies drop shared process identity and the Linear token, and their children get no GitHub credentials. `lf clone` uses the same snapshot. (#1612)
- **Desktop marks preview copies.** `WorkFrame` gains a required, nullable `copy` field. When Desktop's data comes from a copy, it shows a **Preview** capsule at the end of the titlebar breadcrumbs. (#1620, LOO-537)
- **`lf-world` builds isolated fake worlds** through the production writers, with a stand-in `claude` that resumes transcripts and echoes input without needing an account or network access. The recipes are `portfolio`, `one-task`, `wave-backlog` (LOO-514), `task-review` (LOO-515) and `wave-lifecycle` (LOO-500). Each world also has its own `gh`, which answers PR reads from files and refuses every other command. (#1612, #1614, #1622)
- Desktop screenshots and the native performance fixture now run on worlds, and the hand-written `selected` mock and `SessionFixture` are deleted. The opt-in native performance fixture, which had broken on main, passes again. (#1612, #1614, LOO-532)

## Operational notes

- **Schema migration:** the `reader_floor` draft, which adds `schema_migrations.reader_floor`, is released with this version. Builds released before this one don't read floors, so they still refuse a store that a newer release has migrated.
- **Preview copy cost:** the first time a release build uses a new set of drafts, the copy migration took 265 s against an 8.8 GB store. Most of that time was spent sanitizing `processes`. The reader's first frame waits for the copy to finish.
- **Store locking:** read-only transactions on write connections now take the write lock at `BEGIN`, so they can wait behind a writer. WAL reads outside explicit transactions are unaffected.
- **CLI surface:** `lf open` replaces `lf desktop open`. `--waiting` becomes `--available`. New: `lf wt prune --branches`. The `desktop_open_explanations` DTO is now `work_links`.
- **Not yet observed after merge:** a real headless `pursue` reaching each concept-review outcome, an installed headless `lf task run` with no banner in its output, and hands-on Desktop demos of the three new worlds.

## Small changes

- `lf-world` drops inherited `LF_*` variables (except `LF_BIN`), so it works when launched from an lf Session.
- `check_attributegraph_cycles.sh --world` reads the app's own stderr instead of passing falsely. It now reports real cycles on the populated Wave view, which are tracked separately.
- `generate_screenshots.py` captures the app's own window instead of the whole desktop.