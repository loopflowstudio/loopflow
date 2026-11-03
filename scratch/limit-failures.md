# Two kinds of limit failure to recover (Jack Heart, 2026-11-02)

Jack clarified part (a) of LOO-714 covers two distinct failures:

1. **Interactive Task Sessions hit a limit one by one.** Jack can have many Task
   Sessions open, and today each one that hits a usage limit needs an individual
   manual fix: switch the model, or switch to an account that still has usage.
   Wanted: one place (or no place) to do that. A Session that hits a limit should
   pick a model or account with remaining usage itself, and Jack should not have
   to visit each Session.

2. **A Task worker's Graph crashes on a limit and does not retry on another
   account.** A headless step that dies on a limit stays dead. Wanted: the Graph
   retries the step on a different account (or model) automatically, keeping the
   same conversation and work, and waits for reset only when nothing can continue.

Both share the selection rule (which account/model has usage) but differ in who
acts: an interactive Session re-selects in place; a headless Graph re-launches
the step. Design and test both paths.
