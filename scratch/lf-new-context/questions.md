# Open design questions

## Confirmed

- Conversation launch uses the configured app or terminal, without forcing TUI or IDE. The latest direction replaces always-design with a default prompt chosen by the current repo/Wave/Project/Task scope.
- New shell creates a flexible ordinary terminal inside the active worktree workspace.
- Conversation and companion shells switch together; each worktree owns its group.
- Two layout levels: worktree splits outside, terminal splits inside each worktree. Either supports horizontal or vertical splits.
- Visible embedded agents marked ELSEWHERE are a registration bug to address, including manual launches from New shell.
- Main-view owns the unified Task list with compact Wave/Project groups and optional list visibility. The worktree graph is the workspace layout, not a competing sidebar inventory.
- The user reopened taskless sessions after considering the missing-Project case. Mandatory Task-first creation and eliminating an Unfiled section are no longer settled.

## Still open

- Current direction: zoom level supplies the starting conversation's subject and default prompt. Opening it need not create a Task or worktree. Task-first creation and a taskless exploratory checkout remain alternatives, not settled requirements.
- Does this action open/resume the scope's existing human Session or explicitly start a fresh one? Reconcile the word New with main-view's one-current-Session rule without replacing live work silently.
- Exact interactive prompt/command at each level remains open. Existing Wave/Project operate skills are autonomous passes with launch/chat side effects; existing loopflow skill launch forces TUI. Neither behavior should be inherited accidentally by the configured-destination conversation entry.
- If taskless checkout creation remains available, File as task must adopt the same checkout, conversation, and terminals. Its implementation and entry labels remain exploratory.
- Distinguish unfiled workspaces from orphan PM Tasks: actual Task creation still requires an existing Project. Unfiled is a proposed navigation section, not a new planning entity.
- Reconcile multiple independent exploratory workspaces with main-view's one-current-repo-Session proposal. General repo conversation and per-workspace design conversations may need distinct associations.
- Final creation label and placement: Task-backed creation suggests New task; header versus bottom-right footer remains provisional.
- Repeatable native terminal/client binding must be proved before implementation; a one-shot Run receipt cannot cover arbitrary successive commands in a shell.
- Reconcile main-view's one-current-human-Session rule with an unrestricted shell manually starting another independent agent. Preserve every live conversation; no automatic killing or hiding to enforce cardinality.
- Existing genuinely unmatched Sessions need an explicit attachment/recovery route; no automatic Task fabrication or omission on failed planning reads.
- Wave/Project placement remains unspecified.
