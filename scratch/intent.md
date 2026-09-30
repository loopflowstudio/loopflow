# Product direction

Jack Heart, verbatim during the design conversation. Later corrections govern earlier alternatives.

> I want to think of one of the differentiators of Loopflow being we don't want you to be checking on your agents. They work in the background reliably to transfer work between stages for YOU to work. You're not blocking THEM.

> Focus on your own work -- this is stuff like the human-stage-centered view of a workflow being its primary one

> and the nodes become interactive skills and the edges become flows in that view basically

> you *can* zoom in on those edges of course, but we aspire to make it ~automatic and not something you think too much about. That said you should also edit it to match your workflows

> I think editing lives functinonally where lives today, with more to build. Probably need better UI for setting default flow per wave/project. maybe a file editor view for the flow file and make sure it takes affect bc were picking up from wt git

> What if a primary-ish view was just all the agent sessions on a task + any extra terminal + file browser. some ability ot expand collapse each. esp focus on one session

> where do we actually want to use that, and do we want to give just broadstrokes "if at any point you want to, you can raise a chat" to any agent

> vs to what degree we want to have more persistent, long-lived general purpose primary interface agents

> i think this is maybe pointing towards something like a redesign of the wave resident idea but not as a resident service. so some sort of single persistent AgentSession at a per-wave or per-repo level, and having those chats communicate with the blocked lf autonomous sessions

> so i am imagining that the desktop would just immediately initiate sessions at per repo and wave level as soon as it knows those exist

> and this basically replaces the orphan sessions window, which becomes more of like a debug screen

> i think if you ctrl-c that session, we start a new one

> Right, but we keep the interactive flow direct sessions

> but i dont really think we need asks if we do this. otoh if asks are scoped for tasks i dont see why not include them in the same play as the flow session

> one other note -- wantto make sure that sessions are grouped into the task based on worktree, even if not part of a flowsession

> On the contrary, that IS the Ask surface. but we should make sure Asks are designed to fit that UX


> For these new WaveSessions, we'll want to combine design and operate into something that balances the two: emphasize autonomously solving problems to get tasks shipped or progressed to their next human interactive point, but also at any point, be ready to capture a design and turn it into tasks

> repo will also need an even more general prompt should be assumed to be the onboarding experience for loopflow, as well as an agent of last resort.

> $kickoff
> actually maybe make this a task and run a flow with kickoff in it? idk

> ok just run kickoff here then

During review, Jack answered the question about automatically waking the Wave
Session for operational blockers, while keeping planned reviews and decisions
requiring Jack in direct Task Sessions:

> Yes—wake the Wave Session for operational blockers (recommended).

Jack continued, exploring the communication model rather than approving a protocol:

> i think any task session (as defined before, e.g. wt counts) should be able to to somehow "message" its owning wave session, and likewise everyone to the repo session
> but im not sure about it

> how can we minimize this from a product design perspective?
> otoh, you could also think of it as just the wave is reading the tasks' logs
> so the task doesnt need to like have a special channel. it just have various forms of output

> and the wave knows how to read all of them, and likewise the repo

> maybe the repo session theres more of an argument for a push message system

Jack then accepted the proposed three behaviors: Task Sessions produce normal
output; the Wave observes its Tasks and wakes for operational blockers; any Session
can request repo attention with original evidence and a reason. Direct Asks still
bring decisions to Jack, with no general messaging UI or parallel conversations.

> Agreed

Jack then refined the Task's primary conversation and Flow adjustment:

> I am thinking maybe the way we do it is that once a task is launched, it gets a TaskSession. the taskSession agent is responsible for running the flow for the task, and workign with you to adjust the flow as necessary. interactive substeps still get their own window.

Initially answering whether a change should affect remaining work or a new run:

> Adjust the remaining steps at a safe boundary (recommended).

Jack clarified the mechanism and timing; these later statements govern:

> I think it likely often means kill current flow and restart with a new flow

> but could also be "kill at end of loop"

> so "finish, then switch" or "switch now"

Jack clarified exactly where “finish” stops:

> where finish means like get to the inner next loop point and exit instead of decide
