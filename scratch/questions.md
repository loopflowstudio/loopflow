# LOO-604 open questions

- Assumption (2026-10-26): on a slow store, live parts may arrive up to
  `BUSY_SHARE` × projection time late (~8 s for planning on Jack Heart's store)
  in exchange for bounded CPU. User actions confirm via checkpoint, which never
  waits. Reversible: raise or remove `BUSY_SHARE` in `work_watch.rs`.
- Assumption: Waves outside `planning.roadmap` (abandoned/retired or
  authored-only) show objective-only rows in WavesView, without Projects.
  Previously each got a `wave status` read.
- Not done: pid 5210 (orphaned reader, 99% CPU since ~08:30) is still running on
  Jack Heart's Mac; it was left alone. `kill 5210` stops it; the fix prevents new ones.
