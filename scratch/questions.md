# Assumptions (LOO-709)

- **Grace: 2 minutes.** Jack Heart named 2m and 30m as candidates without
  choosing. Use 2m to meet the idle-cost goal; configure through
  `session.end_detached_after`.
- **Crash restart: viewed AND no recorded exit AND gone.** The brief says
  viewed when the machine went down "or that recorded no exit". The narrower
  interpretation avoids restoring every abandoned conversation; busy unviewed
  owners remain resumable explicitly, with no replay.
- **Screen retention: keep existing files.** Operator scope reconciliation and
  Jack Heart's review steer retain the brief's last-screen promise. No expiry
  is selected; optional pruning does not block delivery. The agreed LOO-710
  path/reader cutover and remaining acceptance are in the design.
