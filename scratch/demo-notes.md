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

The design changes these quotes asked for are items 1–4 in `plan.md`.
