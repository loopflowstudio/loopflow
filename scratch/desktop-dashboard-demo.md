# Desktop Dashboard demo attempt — October 18, 2026

Full note: `08253c4dda:scratch/desktop-dashboard-demo.md`.

A populated-clone demo at `e08cbb102d` failed before app launch: clone creation hit
SQLite `SQLITE_IOERR_GETTEMPPATH` (6410); a retry with clone-local `TMPDIR`/
`SQLITE_TMPDIR` progressed but fell below TESTING.md's 32 GiB disk reserve
(8.4 GB database + 6.9 GB WAL, not ready). The clone was disposed; no app was
replaced. The later synthetic-fixture review in
[dashboard-continuity-review.md](dashboard-continuity-review.md) superseded it.

Open engineering recommendations (not agreed UX, not launched work): clone creation
should choose a writable temp dir inside its sandbox and budget its peak footprint;
live `desktop state`/control target `com.loopflow.mac`, so a separately identified
preview cannot show candidate CLI↔native round-trip parity.
