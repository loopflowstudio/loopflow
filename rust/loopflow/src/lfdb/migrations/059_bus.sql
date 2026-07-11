-- The db IS the bus. `lf radio pub` is an INSERT here; every subscriber polls
-- forward from an id cursor. No server sits in the path, so publishing works
-- with zero loopflow processes running and two detached hands hear each other
-- with no served wave.
--
-- The bus is not a log: a sweeper drops rows past a wall-clock window on every
-- publish AND on every read, so a bus that went quiet still forgets on time.
-- AUTOINCREMENT keeps ids monotonic across those deletes, so a cursor never
-- rewinds and `sqlite_sequence` holds a high-water mark the sweeper cannot
-- erase — that mark is how a subscriber sees a gap it can no longer read. The
-- run ledger and PRs remain the records of record.
CREATE TABLE IF NOT EXISTS bus_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel TEXT NOT NULL,
    byline TEXT NOT NULL,
    text TEXT NOT NULL,
    at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS bus_messages_at ON bus_messages (at);

-- A durable subscriber's playhead into the bus. A mind asleep when a hand
-- reported catches up on wake — within the sweep window. Beyond it, the cursor
-- jump is what makes the miss visible.
CREATE TABLE IF NOT EXISTS bus_cursors (
    subscriber TEXT PRIMARY KEY,
    cursor BIGINT NOT NULL
);
