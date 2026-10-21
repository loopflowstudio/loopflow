# 878ecf open questions (kickoff 2026-10-21)

1. **Person key.** Jack said "emails for now", and a committed `person:` value
   needs something a person can type. LOO-555's draft instead uses the random
   `planning_user` UUID as the person key. Assumption: the key is the
   lowercase Git `user.email`. LOO-555 and this Task must agree before either
   lands.
2. **Who runs the edge after a review node.** Assumption: the owner's Machine
   runs it, and the reviewer only chooses it. The alternative is that the
   reviewer runs it. That is a takeover, and remains available via
   `lf task handover`.
3. **Auto-run on return.** Assumption: no. An import never launches work. The
   owner's live Session gets a steer, and a bare `lf task run` uses the chosen
   edge. An opt-in such as a Workflow-level `on_return: run` is possible later.
4. **CLI listing.** The brief names `lf session list --available`, but a handed
   Task has no Session on the recipient's Machine. Assumption: Tasks are
   listed with `lf task list --available`, and Desktop's Available filter
   includes them.
5. **Short names in `person:`.** Assumption: email only for now. A known-name
   resolution exists for `lf task handover --to` only.
6. **Notifying.** Assumption: request a GitHub review only when a PR already
   exists, and never open a PR just to notify. Under Linear, set the assignee.

## Jack Heart's direction (2026-10-21, design review)

- Drop the GitHub review request. The real question is how a handover knows
  about Dustin and Dustin's Machine.
- Addressing: either use the person's account on the planning provider
  (Linear, Asana, Notion), or put a Machine field on the handover and parse
  it; the Machine that matches picks it up.
- Machines can be public (or a person marks a Machine public). Also allow
  "just this user", with a Machine declaring "I am the default for this user;
  I pick it up" (e.g. the laptop).
- Without a provider, put a basic users system into Git. Survey user and team
  records on Asana, Notion and Linear; put their common core into Git and use
  it as the API to all three.
