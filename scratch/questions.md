# Open questions and assumptions (LOO-712)

- **Ownership and input control release at EOF, not after the grace window.**
  Rex keeps a dropped owner for 60 s. Here a Unix-socket drop releases the
  connection's claims; it does not prove process death.
  `automatic_attachment_claims_control_after_the_previous_viewer_disconnects`
  (LOO-618) requires a dead controller's claim to be free at once. The spool
  alone survives the grace; a resume re-takes control/ownership when asked.
- **Events ride stream 1 in pipe mode too.** The viewer strips its nonce APCs
  before writing stdout, so piped stdout stays byte-exact. A second out-of-band
  control channel for pipes was rejected: one stream, one count, one spool.
- **Theme is relayed opaque.** The CLI viewer never sends one; Desktop (LOO-672)
  owns the colors it reports. Included because the Task names it.
- **Viewer name** for "who took it" is the viewer's tty (`ttyname(0)`) when it
  has one; Desktop can pass its own.
- **`status` names its program as `program`, not `id`.** The request envelope
  already uses `id` for the request number; with `#[serde(flatten)]` both landed
  on one JSON key and every status report failed to parse silently (the owner
  drops a connection whose frame does not parse). A unit test in `wire.rs` pins it.
- **One `resize` event per attach.** A `take` on attach produces the owner's
  resize for every viewer in the same pass, so the attach-time resize is sent
  only when no owner change is pending.

- **Successor integration before dependency landing (2026-11-02).** The implement
  iteration ports LOO-705's command and follow behavior into this branch's
  existing retirement path. It leaves retirement-policy changes in LOO-705 and
  preserves Jack Heart's landing order; the later integration retains that
  policy on the single control-event path.
