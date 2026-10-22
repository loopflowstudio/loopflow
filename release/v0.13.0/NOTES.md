# v0.13.0

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.0 brings the completed 0.12 cycle together around keeping work intact through planning changes, review, recovery, and delivery. Conversations and Flows resume from durable Session records, Desktop brings Tasks and their files beside the conversation, and publication failures preserve the work needed to continue. These notes cover progress since v0.12.0, including features already delivered in patches.

## Continue work without reconstructing it

Tasks can continue through review and revision using their saved Flow, while durable Session records bring conversation identity, command outcomes, and Flow progress together. Recovery increasingly preserves the existing work instead of requiring another launch to rebuild its context.

- Resume conversations without injecting a new participant-update prompt.
- Keep a Task’s selected agent and retain account choices across Flow resumes; stalled work is more visible.
- Route Task direction through Linear comments and keep routine agent progress out of that steering channel.
- Inspect Linear Tasks before starting work and adopt existing branches.
- Finish or cancel Tasks with coordinated PR and branch cleanup.

## Keep the conversation and its work together

The Mac workspace evolved from separate activity and Session surfaces into one repository → Wave → Task → Session outline. Navigation, Flow inspection, and file editing now support working from the conversation itself.

- Find work with ⌘K and Task links.
- Inspect Task Flows, Monitor, comments, and named Sessions in the shared workspace.
- Read and edit Task files beside the conversation.
- Preserve live terminals while navigating, with shell command blocks for terminal work.
- Inspect composed Flow templates in a folded preview instead of opening every step at once.

## Change plans and preserve direction

Resumable chapters replace project selection, with chapter review and plan-start skills supporting explicit planning boundaries. Later changes simplify Wave operation: resident controllers give way to bounded Task workers, and the resident Wave service and chat surfaces are removed.

- Review chapter evidence and reset plans through dedicated skills.
- Keep chapter sweeps and planning refresh working around other Teams’ Projects.
- Consolidate planning commands and delete Linear Tasks through `lf`.
- Realign plans and code before publishing Task work, and preserve useful learnings in Wave memory.
- Use the manual S1–S5 operation skills for Wave coordination and governance.

## Deliver without losing the branch or PR

Delivery recovery now retains more of the identity and evidence needed to continue after failure. Integration also shifts toward preserving authored history, while queued PRs keep their existing heads and CI progress.

- Merge Task branch updates without rewriting authored history.
- Retain Task PR identity through publication failures and preserve published descriptions during final preparation.
- Recover blocked landing operations on the same commit while exposing the repair agent’s concrete blocker.
- Continue watching queued PRs without redundant rebase recovery, and preserve CI while armed PRs await queue entry.
- Reuse the merge queue’s exact CI proof on main.

## Spend less time waiting and diagnosing launches

The cycle reduces repeated verification work and makes launch inputs and account state easier to inspect. Desktop acceptance checks run headlessly, with acceptance consolidated at gate.

- Discover commands, skills, and Flows without launching them.
- Inspect and configure context budgets before launching agents.
- Use an installed coding agent for unconfigured runs, replacing the earlier unconditional Codex default.
- Unify account authentication, remember browser profiles, refresh account status, and keep managed Codex accounts tied to the intended login.
- Improve prompt-cache reuse and expose cache hit rates alongside retained usage evidence.
- Reuse Rust, Swift, and Xcode build work in CI; reduce repeated migration bookkeeping and bound busy cache cleanup.

## Operational notes

Upgrade and release recovery receive corresponding safeguards. Branch builds use private data, installed Tasks stay on their owning runtime, and saved Session launches remain tied to their original installation.

- Failed install switches fall back to the prior installation. Release downloads are pinned, and the legacy refresh entry point remains available for older installed CLIs.
- Installation and machine commands work independently of a repository checkout.
- Minor releases pair with a closing patch and summarize the completed cycle’s product snapshot.
- Release recovery handles same-tag publisher worktrees and withdrawn release PRs. Scheduled release CI failures reach the landing repair supervisor, and cron retains failure outcomes.
- macOS release checks verify that app resources are self-contained.

## Small changes

- Terminal catalog output stays aligned.
- Failed searches no longer falsely block Task handoff, and saved plans avoid activating unrelated skills.
- Requests and saved artifacts preserve people’s names.
- Recent execution queries avoid parsing unrelated historical events.
- CI job logs record CPU and memory use.