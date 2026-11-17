# Direction for the Session commands Task

Jack, 2026-11-17, in the LOO-924 conversation:

- "lf resume shouldnt take messages, i dont think?" / "that seems like a mixed API"
- "resume connects to existing sessions"
- "i feel like that space ends up with a lot of places where simialr ideas are spelled in several differnt ways"
- On steers waking a stopped Session: "desirable, what is the cost?"
- "id say separate the two costs, but stack on top and start them in parallel now"

The cost answer given: cheap to build (the `--message` path already starts one
headless turn on a saved thread for all four harnesses); the price is in
behavior if any steer can wake: a Task steer reaches every Session of the Task
(LOO-924 had about 55 closed ones), a woken step shares a checkout with a live
Graph step, and something has to start the process. Hence: only a steer naming
one Session wakes.

Review page with the full table and six questions: `scratch/session-commands-review.html`.
Questions 3 to 5 there (open vs resume, fresh-thread verb, selectors) have no answer yet.
- On waking only for a steer that names one Session: "ok, eakign only directly named steers is fine"
