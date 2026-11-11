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

`native_input.rs` stays as designed until a person picks one.
