# v0.13.11

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.11 makes planning usable without Linear and separates finishing a Task from finishing its Workflow. Agent launches put the request first, refresh checkout context instead of freezing it at startup, and give conversations recognizable names. Provider-process tracking also becomes more explicit, with compatibility changes and remaining acceptance limits worth checking before upgrading a running installation.

## Keep planning when Linear is unavailable

Waves, Projects and Tasks save locally, so creating, editing and starting work no longer requires Linear. Planning identity and Workflow definitions live in SQLite; Wave goals and memory are read directly from the selected checkout, not from stored document copies.

- Create a Task with `lf task create --title "Fix the parser" --json`, edit it by its returned ID, then prepare it with `lf task checkout <id>`. IDs exist before checkout, and unique prefixes resolve saved Tasks.
- Local edits, comments, ordering, refiling and chapter rotation retain delivery records for optional Linear synchronization. Repository-wide synchronization runs while a CLI or Desktop foreground connection is active.
- CLI and Desktop projections expose pending changes, uncertain attempts, errors and retained conflicting values. Observed Linear conflicts win without moving local Workflows or controlling running Processes.
- Edit `wave/<name>/GOAL.md` or `MEMORY.md` directly. Status and context read fresh checkout files, including ancestor Markdown. Missing files are created without overwriting existing content; registry renames leave authored directories in place.

Linear synchronization is not conflict-free replication: an unseen concurrent edit can still be overwritten. Initiative summaries use one committed local default-branch revision, leaving dirty and unmerged edits local. Sync does not fetch; missing goals or a missing default-branch ref block mutation, and descriptions retain the 255-character limit.

## Finish one Task and carry remaining work forward

Tasks now support zero or one PR. Completion records the outcome independently of Workflow position, while accepted obligations after a merge become linked follow-up Tasks rather than additional PRs on the original Task.

- Use `lf task complete` and `lf task reopen` without moving the Workflow or rewriting Process history. Reaching Workflow `end` requests completion; a failed request can retry without replaying the Flow.
- Finish work without publishing a PR while retaining files and commits. Use stacked Tasks for dependent changes; `--design` transfers the child brief without overwriting later edits.
- Merged Tasks remain open until follow-through files or links remaining obligations, or records why none are needed. Filing retries preserve the follow-up's identity and destination Project.
- The `ship` Flow waits with `lf land --wait-and-fix`, then performs follow-through. Interrupted finishing work can resume without repeating gate or re-arming a merged PR.
- CLI and Desktop projections show delivery state, follow-up links and completion retry reasons. Historical multi-PR records remain read-only history.

Follow-up dates do not install schedules; due work returns on the owning Wave's next pass. Optional Linear delivery continues independently after source completion.

## Start with the request, keep context current

The skill, request and clipboard now arrive in the first turn instead of being buried in a large instructions payload. Provider hooks separate fixed instructions from refreshable conversation context across Claude, Codex and OpenCode.

- Startup and compaction reread checkout Wave documents, ancestor documents and scratch within 10,000 UTF-8 bytes. Whole files or marked opening excerpts retain references to complete sources; compaction restores the saved active skill.
- Documentation and branch changes arrive as references for inspection rather than preloaded bodies. Memory and scratch budgets become authoring targets, not launch gates.
- New Sessions take short names from the requested work, with Task identifiers first when bound. Interactive launches set an initial terminal title; cmux workspace and tab names follow Session renames. Other terminals pick up changed names on reconnect.
- Codex capture uses a temporary profile that preserves terminal-wrapper hooks and saved configuration. Missing capture is reported without attaching unrelated history, and optional missing capture payloads no longer discard native resume identity.
- Flow output emphasizes step labels and agent messages. Context accounting and INFO diagnostics move behind `-v` / `--verbose`, which propagates through steps; retained prompt files no longer collide when launched within the same second.

Native naming hooks cover plain Claude launches and trusted shared-server Codex TUI launches in Loopflow repositories. Embedded Codex TUI and `codex exec` naming remain unsupported. The cmux rename demonstration used a provider stand-in; rendered host acceptance and the installed setup remain unverified.

Context-delivery observations cover Codex headless and Claude headless/interactive launches. End-to-end interactive Codex, compaction, resume and OpenCode acceptance remain unverified.

## Run a Flow from the conversation

Skill synchronization exports Flows as readable native chat recipes: headless work runs through Loopflow, while reviews and loop decisions stay in the current conversation.

- Recipes include commands, loops, branches and conversational review steps. Re-sync skills after editing a Flow or adding repository overrides.
- Exact literal names resolve before normalized fallbacks. Slash and dash definitions remain distinct; Claude gives an explicit dashed definition its literal name.
- Flow exports have no prefix and skip names already owned by Loopflow or third-party skills, preserving native controls and existing bundles.

Automated checks cover recipe generation and execution contracts; native Claude/Codex selection and adherence have not been demonstrated.

## Inspect and clean up the processes that own work

Provider processes now share a durable inventory used by activity views, Task cleanup and orphan cleanup. An unanswered historical turn is no longer enough to block checkout cleanup, but a Process that is not provably dead still can.

- Each AgentProcess records its launching process, Session, PID and birth time before provider code executes. Replacement preserves earlier records.
- Fresh attachment tokens prevent stale clients from writing or stopping a provider after handoff, including when an earlier owner reconnects.
- Cleanup uses recorded identity and fresh operating-system evidence. Unresolved processes stay visible as Unknown; failed observation reports an error rather than inventing completion.
- Loopflow-owned conversations and processes are named `LfSession` and `LfProcess`. Opaque provider conversation IDs use `AgentSessionId` across resume, history and account attribution, preserving earlier selections in immutable history.

The provider lifecycle cutover is incomplete. Reaping covers noninteractive providers; the reusable FIFO lifeline is Codex-only. Claude/OpenCode takeover and stop behavior, configured native connections and installed live-provider cleanup still need verification. Earlier orphan-cleanup work does not establish those broader guarantees.

## Operational notes

Several interfaces change in this release; update scripts and account for live runtime artifacts before upgrading.

- Durable process identity is `LfProcess.id` / `LfProcessId`, with references such as `parent_lf_process_id`; `pid` remains the Unix process ID. JSON fields and runtime artifact names change without old-name readers. Stored IDs, outcomes and history are preserved by migrations, but earlier builds' live journal receipts and Codex lifelines are not adopted.
- `lf wave edit` is removed; edit checkout documents directly. Retired goal/input budget settings and `lf context --skill` are removed. Terminal first turns reaching 122,880 bytes fail before provider startup with a size/cap error.
- `--tui` is removed. Direct terminal launches remain interactive, `-i` forces interactive execution, and `-b` remains headless, including Flow steps using the default skill.
- `lf account codex use` reconciles the shared Codex app-server even when the saved login already matches. It installs the login before waiting, allows running turns up to five minutes, then warns and may interrupt remaining turns when restarting. An unreadable daemon login returns an error without restarting; failure leaves the selected login installed for retry.
- Loopflow Codex terminals launch with `--no-daemon` and retain their own login until restarted. This flag was verified with codex-cli 0.161.0; older versions without it reject launches. Launch-time account switches leave the shared daemon alone. The final grace-period behavior still needs a configured-provider walkthrough.

The included evidence does not establish a passing full release matrix. Outstanding checks include installed migration and provider acceptance; recorded branch suites also retain install-retention, checkout-watch and planning-identity migration failures.

## Small changes

- Agent startup avoids redundant provider probes, unnecessary ambient Wave resolution and repeated directory resolution. Paired warm bare-launch measurements showed median gains of 41 ms for Claude and 24 ms for Codex from the first two changes; final all-change native and cold-cache gains are not established.
- macOS installation distinguishes unrelated running development CLIs whose binaries were deleted from actual upgrade targets. Ambiguous targets still block promotion, and unresolved-target errors include the launch path.
- The homepage now presents conducting agents, Waves, Tasks and built-in Workflows, with Mac download and CLI installation together. Some examples remain ahead of the product: `lf graph`, `lf usage --wt` and illustrated account, usage and monitor output are not delivered by the page redesign.
- Dependency updates include shlex 2.0.1, uuid 1.27.0, libc 0.2.190 and Ruff 0.16.10.