# Glossary

Look up a word encountered in the app or these docs. Loopflow's own words
come first, followed by the engineering terms used in commands and explanations.

## Loopflow's words

| Term | What it means |
|---|---|
| **Skill** | One step: instructions for the AI, written as a text file in `.lf/skills/`. |
| **Flow** | Steps in order, written as a file in `.lf/flows/`. |
| **Loopflow** | A flow that can go back and try again. Exactly: a flow with at least one backward edge. |
| **Deciding step** | The step that chooses between moving on and going back. |
| **Human step** | A step that waits for a person. In a flow file it is marked `human: true`. |
| **Task** | One piece of work with a finish line. |
| **Wave** | A goal Loopflow keeps working on. It has a written objective, a memory, and a schedule. |
| **Chapter** | One planning period for a Wave. Each chapter has its own plan. |
| **Project** | The internal record holding one Wave chapter’s plan: KRs, metric targets, and Tasks; select the Wave to work with that plan. |
| **KR** | Key result: an outcome and the evidence that would prove it over a stated period. |
| **Memory** | A text file, `MEMORY.md`, where a Wave writes down what it learned. |
| **Session** | One open conversation with the AI, shown as a terminal inside the app. |
| **Run** | A record of one time the AI coding tool was started: what it was told and what happened. |
| **Home** | One computer running Loopflow, with its own records. |
| **Steer** | Saved direction for a Task, carried by a Linear comment; an idle Task keeps it without starting. |
| **Harness**, **provider** | Harness means the AI coding tool Loopflow drives; provider can mean that tool or the external service supplying AI, planning, or chat. |
| **Scratch** | The `scratch/` folder, for working notes. Delivery preparation clears it before submission or landing, so keep durable conclusions elsewhere. |
| **Work** | A saved planning record for a Wave, its internal chapter Project, or a Task; it survives the processes that work on it. |
| **Context** | Information supplied alongside an AI’s instructions, such as source files, operating rules, and working notes. |
| **Ask** | A request that opens a Session with a person and blocks its caller until that conversation is explicitly completed. |
| **Ready, complete** | For a Session, ready means feedback is saved and complete means the reviewer ended it; for Work, ready means unfinished. |
| **Condition, execution** | A Task condition says what it needs; execution says whether its worker is starting, running, waiting, blocked, idle, or unknown. |
| **Roadmap** | Tasks grouped by what needs attention, combining the current plan with execution evidence. |
| **Placement** | The recorded Home assigned to Work; it does not itself start a process or open a connection. |
| **Keeper, resident, listener** | The keeper is the Home’s background service; a resident stays available, and a Wave listener receives messages and starts turns. |
| **Heartbeat, cadence** | A heartbeat periodically checks for work; cadence is how often recurring work is attempted. |
| **Chord** | A group of related Waves, including nested Waves in the same repository. |
| **Viable System Model** | A way to group responsibilities into operations, coordination, control, intelligence, and identity, used by the s1–s5 goal templates. |
| **Metric** | A defined measurement collected repeatedly to judge progress. |
| **Instrument** | In a metric contract, the product code that collects observations; separate from the description of Loopflow as a software instrument. |
| **Target, window, freshness** | A target is a desired measured result, a window is the period measured, and freshness limits how old usable evidence may be. |
| **Contract, revision** | A contract states what a skill or measurement promises; a revision identifies the exact version of that definition. |
| **Portfolio** | The collection of chapter plans or measurements shown together for a Wave. |
| **Manifest, bundle** | A Run manifest saves launch settings before the coding tool starts; its bundle holds that manifest and later evidence files. |
| **Receipt, ledger, journal** | A receipt records one event or result; a ledger or journal keeps such records in order. |
| **Replay** | Starting a new Run from a previous Run’s recorded request and settings; it does not resume the source Run. |
| **Invocation, occurrence, pass** | An invocation executes a Flow once; an occurrence identifies a step in it; a pass counts visits through a loop. |
| **Backward edge** | A declared route from a deciding step back to an earlier step, repeating the steps between them. |
| **Router, XOR** | A router chooses which Flow path runs; XOR means exactly one of the available paths is chosen. |
| **Flow position, cursor, playhead** | The saved place in an executing Flow, used to continue from the correct step. |
| **Op** | A mechanical Loopflow command declared as a Flow step, such as `op: pr land`. |
| **Account, subscription** | An account is a login to a coding-tool service; its subscription determines the available paid usage. |
| **Account route, selector** | A route lists logins to try in order; a selector identifies one login, often by an unambiguous email prefix. |
| **Access profile** | A saved choice of browser profile used to sign in; it does not choose the account that spends usage. |
| **Pin** | A recorded association that stays fixed, such as the account owning a conversation or the Flow definition used by an invocation. |
| **Disposition, rotation** | Disposition says what happens after merge; rotation moves a Task to its next PR or a Wave to its next chapter. |
| **Slice, boundary** | A slice is a coherent part of work; an execution boundary is the point where a saved step starts, stops, or waits. |

## Borrowed from software engineering

| Term | What it means |
|---|---|
| **Terminal** | A window where you type commands instead of clicking. |
| **Command line**, **CLI** | A program you use by typing in a terminal. Loopflow's is `lf`. |
| **Repository**, **repo** | A project folder whose history is tracked by Git. |
| **Git** | The tool that saves committed versions of a project so they can be inspected, compared, or restored. |
| **Commit** | One saved change, with a note saying what changed and why. |
| **Branch** | A separate line of changes, kept apart from the main version until it is ready. |
| **Main** | The branch that holds the finished, agreed version of the project. |
| **Worktree** | A checkout on its own branch, keeping one Task’s edits apart from another’s; contributors within the same worktree still share files. |
| **Pull request**, **PR** | A proposal to add a branch's changes to main, where they can be reviewed first. |
| **Merge**, **land** | Adding a branch's changes to main. |
| **Rebase** | Replaying your changes on top of the newest main, so the two fit together. |
| **CI** | Continuous integration: automatic checks that run on every pull request. |
| **Test** | A small program that checks another program does what it should. |
| **Lint** | An automatic check for mistakes and style, without running the code. |
| **GitHub** | A website that stores repositories and hosts pull requests. |
| **Linear** | A planning tool. Loopflow keeps Tasks and Projects there. |
| **Token** | The unit AI tools count text in, and charge by. Roughly three quarters of a word. |
| **YAML**, **Markdown** | Two plain text formats. Flows are written in YAML; skills and docs in Markdown. |
| **AI agent, model** | An AI agent uses a model to interpret instructions and call tools; the model is the system generating its responses. |
| **Prompt, turn, transcript** | A prompt is input to an AI, a turn is one stretch of interaction, and a transcript records the conversation. |
| **Shell, Bash, Zsh** | A shell interprets terminal commands; Bash and Zsh are shells. |
| **TUI, TTY, PTY** | A TUI is an interface in a terminal; TTY denotes a terminal connection, and PTY a software-created terminal connection. |
| **IDE** | An app for editing and running code; Loopflow’s `--ide` selects the coding tool’s app. |
| **Interactive, headless, batch** | Interactive work accepts live conversation; headless or batch work runs without someone typing replies. |
| **Argument, flag, argv** | An argument is a value passed to a command, a flag is a named option, and argv is the list of command arguments. |
| **Standard input/output, stdin/stdout, pipe** | Standard input and output are a program’s incoming and outgoing text; a pipe (`\|`) sends one command’s output to another’s input. |
| **Environment variable, PATH** | A named setting inherited by a program at launch; PATH lists directories the shell searches for commands. |
| **Glob** | A filename pattern such as `*.md` that selects matching paths. |
| **Frontmatter** | A settings block at the beginning of a text file, usually YAML between `---` lines. |
| **JSON, DTO** | JSON is a structured text format for data; a DTO is a defined record shape exchanged between programs. |
| **API, SDK** | An API is an interface programs call; an SDK supplies libraries and tools for writing those callers. |
| **HTTP, REST, endpoint** | HTTP carries web requests; REST describes a style of HTTP API; an endpoint is the address handling a particular request. |
| **HTML, URL** | HTML describes a web page’s structure; a URL is the address used to reach a page or resource. |
| **Content negotiation** | Choosing a response format from a request header, such as `Accept: text/markdown`. |
| **MCP, plugin** | MCP connects AI tools through a common protocol; a plugin is an installed extension that may provide tools or instructions. |
| **Authentication, authorization, auth** | Authentication proves who is signing in; authorization determines permitted actions; auth can refer to either. |
| **Credential, secret** | A credential proves access, such as a password or token; a secret is sensitive information that must not be exposed. |
| **Access token, refresh token, OAuth** | An access token permits requests, a refresh token can obtain new access tokens, and OAuth connects accounts through an authorization flow. |
| **Browser profile** | A browser’s separate set of logins, cookies, and settings. |
| **Keychain, secret service** | Operating-system facilities for storing credentials with restricted access. |
| **Doppler** | The secret manager used to supply credentials to commands without putting values in repository files. |
| **Broker, socket** | A broker mediates access for another process; a socket is an endpoint through which processes communicate. |
| **SSH, origin, target** | SSH runs commands over an encrypted connection; in `lf ssh`, origin is the calling machine and target is the receiving machine. |
| **SSH agent forwarding** | Allowing a remote machine to ask the local SSH agent to authenticate without copying its private key. |
| **Local, remote, forwarded** | Local means this machine, remote means another machine, and forwarded means made available from one machine to another. |
| **Hostname, IP, NAT** | A hostname names a network machine, an IP address locates an interface, and NAT lets machines share a public address. |
| **Loopback, firewall, proxy** | Loopback connects only within one machine; a firewall filters network traffic; a proxy handles requests on another service’s behalf. |
| **Operating system, OS, Unix user** | The OS manages programs and resources; a Unix user is the account whose permissions a program uses. |
| **Process, PID, process tree** | A process is a running program, a PID identifies it, and a process tree shows which programs started others. |
| **Daemon, service, launchd** | A daemon or service runs in the background; launchd manages services and scheduled jobs on macOS. |
| **Cron** | A schedule expressed as time fields for recurring commands. |
| **Foreground, background, detached** | Foreground work stays attached to its caller; background or detached work can continue after that caller returns. |
| **Tmux, screen, nohup** | Tools used to keep terminal programs running after their original terminal closes. |
| **Sandbox, containment, execution boundary** | Controls limiting the files, processes, or networks a program can reach; permission prompts alone do not supply containment. |
| **Container, VM** | A container isolates parts of the host system; a virtual machine runs a separate operating system. |
| **Mount, Unix permission mode** | A mount exposes a filesystem at a path; a permission mode such as `0600` specifies which users may access a file. |
| **Lock, lease** | A lock excludes conflicting operations; a lease grants limited ownership that can expire or be checked before use. |
| **Webhook, signature** | A webhook reports a service event through a web request; a signature helps verify its sender and contents. |
| **Encryption at rest** | Encoding stored data so reading the file alone is insufficient without the key. |
| **Telemetry** | Measurements collected while software runs, such as usage, duration, and failures. |
| **Rate limit, quota, cooldown, backoff** | A rate limit or quota caps usage; a cooldown or backoff delays another attempt. |
| **Fallback, failover** | Trying another permitted option when the preferred one cannot be used. |
| **Append-only, immutable, durable** | Append-only records gain entries without rewriting old ones; immutable values stay fixed; durable data survives process restarts. |
| **Database, SQLite, schema, migration** | A database stores structured data; SQLite is a file-based database; its schema defines structure and a migration changes that structure. |
| **Cache, snapshot, projection** | A cache saves data for reuse, a snapshot captures it at a time, and a projection presents selected facts from other records. |
| **UUID, identifier, slug** | An identifier names a record; a UUID is designed to be unique; a slug is a readable name suited to paths. |
| **Idempotent, atomic, transaction** | Idempotent operations can be retried without duplicating effects; atomic changes happen together; a transaction groups changes into one operation. |
| **Fail closed** | Refuse an action when required evidence or enforcement is unavailable, rather than assume permission. |
| **Exit code, no-op** | An exit code reports a program’s outcome, usually zero for success; a no-op succeeds without making a change. |
| **Git checkout, tracked, untracked** | A checkout is the files at a selected Git revision; tracked files belong to Git history, while untracked files have not been added. |
| **Diff, patch** | A representation of changes, usually showing removed and added lines. |
| **Stage, index, dirty** | Staging selects edits for a commit in Git’s index; dirty means a checkout contains uncommitted changes. |
| **Head, base, ref, tag** | Head is the current branch tip, base is the commit built on, a ref names a commit, and a tag gives a version a fixed name. |
| **Git remote, origin, upstream** | A remote is another copy of a repository; origin is its usual configured name; upstream is the branch tracked for shared updates. |
| **Fetch, push, fast-forward** | Fetch downloads Git history, push sends it to a remote, and fast-forward moves a branch along existing history without combining changes. |
| **Conflict, stash, rerere** | A conflict needs a choice between overlapping edits; a stash saves uncommitted edits; rerere reuses a recorded conflict resolution. |
| **Checkpoint, squash** | A checkpoint saves current work; squash combines several commits into one with the same final files. |
| **Stacked PR, serial PR** | A stacked PR builds on another unmerged PR; serial PRs deliver successive parts of one Task. |
| **Auto-merge, merge queue** | Auto-merge asks the host to merge when requirements pass; a merge queue orders and checks waiting changes. |
| **Review, QA, smoke test** | Review examines a change, quality assessment checks its behavior, and a smoke test checks that a basic path runs. |
| **Release, deploy, artifact** | A release publishes a version, deployment runs it in an environment, and an artifact is a built or generated file. |
| **Semantic version, manifest** | A semantic version uses major.minor.patch; a package manifest records details such as name, version, and dependencies. |
| **Hook, workflow, publisher** | A hook runs at a defined event; a workflow orders jobs; a publisher has the access needed to sign or upload a release. |
| **Hash, checksum, notarization, DMG** | Hashes and checksums identify bytes; notarization asks Apple to check signed software; a DMG is a Mac disk image. |
| **UTF-8, symlink, Git blob** | UTF-8 encodes text, a symlink points to another filesystem path, and a Git blob stores file contents. |
| **Stack trace, dependency** | A stack trace lists calls leading to an error; a dependency is software another program needs. |
| **npm, npx, uv, Homebrew** | Package tools: npm installs JavaScript packages, npx runs them, uv manages Python projects, and Homebrew installs macOS or Linux tools. |
| **Linear Initiative, Team, issue, PM** | An Initiative groups Projects, a Team owns issues, an issue records a Task, and PM refers to planning management. |
| **Discord guild, bot, Gateway** | A guild is a Discord server, a bot is a program’s account, and the Gateway connection delivers live events. |
| **Compatibility shim** | Code preserving an older interface while the underlying implementation changes. |
| **SQL** | The language used to query or change a database, including its schema. |
| **UI, GUI, LLM** | UI means user interface, GUI a graphical interface, and LLM a large language model used to generate text and tool calls. |
| **Server, client, protocol** | A server handles requests, a client makes them, and a protocol defines their message format and sequence. |
| **Session epoch, segment** | A saved period of a Wave conversation using one chat provider; an epoch ID selects that period. |
| **Provenance, attribution, lineage** | Provenance says where a fact came from, attribution which Work it concerns, and lineage which launch caused another. |
| **Fixture, end-to-end test** | A fixture supplies a known test setup; an end-to-end test exercises a path across the participating components. |
