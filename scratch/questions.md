# Open questions

- #1890's PR body says "Bare interactive conversations receive updates at their next boundary but are never woken". On this branch a bare interactive launch no owner holds gets only the two SessionStart context hooks (no UserPromptSubmit/Stop), so it gets no Task updates between turns. `only_managed_claude_launches_accept_inbox_messages` now asserts the branch's rule. Decide whether bare terminal launches should keep UserPromptSubmit for update delivery.
