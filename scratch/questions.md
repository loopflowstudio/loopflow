# LOO-672 open assumptions

Concept review, November 7: “auto open everything *AND* prefetch everything” (November 5) came
before “make it 16”, which answered the disclosure that prefetch is "bounded rather than literally
every Task". The later words win: load opens sixteen Tasks, what is on screen included, and goes past
sixteen only when one page alone holds more; Task seventeen opens when reached, not before. To open
every Task on load instead, drop the bound in `prefetch` and keep it for eviction only; a load then costs one provider and one turn per Task.
