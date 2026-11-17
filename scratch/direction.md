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
- On Task steers: "if you steer a task the task conversation should ~always be awake anyway and it can continue from there". So a Task steer needs no wake rule: the Task's conversation is managed, the server already wakes it when idle, and it decides what to do next, including steering a worker by name.
- On a closed Task conversation: "but hte machine server manageds the terminal". A managed conversation keeps running in its held terminal with no viewer, so a Task steer reaches it. It is down only when the agent itself was quit or its process died (a Machine restart). Check: does the server restart a dead managed conversation today, or wait for `lf resume`? If it waits, the server restarting it in a held terminal from the saved thread is the candidate answer to "who starts the process".
