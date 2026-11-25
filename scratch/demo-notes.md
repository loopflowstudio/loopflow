# Demo notes · LOO-1078

## Jack Heart (2026-11-24)

> ok, pausing a task should not close its task session, and it shouldnt take it outside the working tasks list

> we should stack on top of 1063 and then we should add back somet logic similar to what i had before of:
>
> maybe there should be 3 sections in the working task list on the left:
>
> - Active: 
> - Shippping: auto mode or no human step left before landing [this may require rethinking verify]
> - Paused: new

> its owned builds (cargo/rustc),
>
> Right but not so specific -- just any subprocesses of the worker session

> 1b lets make verify an automatic step for now, and rewrite it to do wahtever it can *quickly*, dont block indefinitely e.g. for a release to production
>
> 2 - whatever works
> 3b
> 4 im done with until the next slice is finsihed

> i dont really get what Run | Pause means

> like if its not paused souldnt it be running?
> the distinction i guess is if its at a human step

> but basically it should either say Run: <step after human> or Running: xxx or something

> why doesnt Auto just happen right way

> er taking auto off
> why do we have to wait for a "request"
> What does "unpase without running" do

> [Screenshot 2026-11-24 at 10.23.03 PM.png] pill and Auto are too close together
> Can we just do a quick audit for all actiosn you can do and make sure they all go othrough pending chagnes

> I feel completing a task might need a notice for compelted x task with an undo button or something
> i also think with this we can probably delete the ellipsis menu

> still wonder if theres a way to better integrate this [Screenshot 2026-11-24 at 10.24.34 PM.png]

> 1 : I think just delete the ... completely from the code, not just ide it from view
>
> 2. Paused can still be available i think, the task session and the worker are differnet. agreed that this needs ome holistic thinking though

> lets try C but should it still be a ill
> i agree that keeign the play button locked but allowing specific steps to grow is the right compromise
> *not sure about it being a pill, though i like the way we tied the button to the whole workflow

> lets do c1 an build it

> ok lg2m ship it

The design changes these quotes asked for are items 1–4 in `plan.md`.
