Run one loop iteration against this wave's live tasks.

Read the live tasks, pick the next useful move, dispatch the appropriate flow, and
leave the wave closer to done. If no safe move remains, record the blocker
instead of inventing work.
