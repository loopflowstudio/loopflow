# LOO-823 open: which input holds a detached owner

Today any byte that isn't plain text or a recognized terminal reply makes the
owner's input `Uncertain`, which holds it past `end_detached_after` until
input is cleared. 78 of ~100 resident owners on 2026-11-10 were held this way.

1a. Keep it: any unknown key holds the owner; the troubleshooting doc tells
    people to `lf resume` and clear it.
1b. Cursor and navigation keys on an empty editor (Left, Right, Down, Home,
    End, PgUp/PgDn) leave input empty; Up, Esc, edits and text still hold
    (Up recalls the last prompt in Claude).
1c. Keep 1a's rule, but an owner held only by `Uncertain` (no recorded text)
    ends at rollover after a fixed age; a typed draft still holds.
1d. Keep 1a's rule and show "held by input" in `lf process list` and Desktop;
    nothing ends on its own.

The same hold reaches finished Tasks. Read 2026-11-10 18:55: LOO-775, LOO-776,
LOO-812, MTG-252 and MTG-268 are done, their `task_retirements` rows were due
26-33 h earlier and still `waiting` with "Requested work, a draft or a pending
request still holds this Task". Each Task's interactive owner (PIDs 80631,
4996, 63058, 78405, 38179) has `native_input`
`{"state":"Uncertain","escape":[],"working":false}`: no text. Six Desktop
shells and five checkouts stay behind them. 1b or 1c would release these;
1a and 1d keep them until someone resumes each conversation.

`native_input.rs` stays as designed until a person picks one.
