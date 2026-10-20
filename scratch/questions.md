# LOO-538 open questions and assumptions

- Default `disk_budget` is 20G for every Machine (the steer says the user sets it;
  a default makes rotation work without setup). Jack's Home floor is ~28G
  (database 8.2G + accounts 18G), so it stays over budget after rotation.
- `accounts/` (provider homes; codex `engineering` alone is 12G: sessions 6.2G,
  logs_2.sqlite 1.4G, thread_history 1.4G, packages 1.9G) is reported, never
  rotated: it is provider-owned and holds native history used for resume. Pruning
  old provider sessions needs its own decision.
- Legacy `~/.lf` files from deleted features (`lfd*.db`, `maestro.db`,
  `loopflow-pr872-demo*.db`, `traces.pre-baseline-*.bak`) count as "other" and
  are untouched; their backups rotate, keeping the newest per database.
- Rotated Session captures lose replay/context detail for old Sessions; readers
  already report "no retained capture". Native resume does not depend on them.
- The script empties Trash items older than 30 days when readable (Jack's list
  said "and so on"; Trash was named only as an FDA problem). Docker is report-only.
- Install on the laptop and mini-heart after landing: the launchd plist points at
  the main checkout's script (`scripts/disk-cleanup.sh install` on each machine).
- Consequence on Jack's laptop at the default budget: the floor exceeds 20G, so
  every hourly rotation removes all captures and logs older than a day. Raise
  `disk_budget` (e.g. 40G) to keep more history.
