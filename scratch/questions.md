# LOO-589 notes

- Assumption: the fix lives in the test fixture, not WorkModel. The hang was the stub gating every
  query (including the bound workspace's background pane-layout restore) on one barrier key, and the
  barrier kept one continuation per key, so the second waiter stranded the first. Production
  behavior (cancel during a Task read) is unchanged and remains covered.
- Assumption: the continuation-misuse CI guard stays in LOO-589 rather than a separate Task; it is a few lines and the suite only becomes clean when this fixture fix lands.
