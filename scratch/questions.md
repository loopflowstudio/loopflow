# Readable task identities — open assumptions

2026-10-13. Jack Heart accepted the design and requested `pursue`. The following
implementation assumptions apply unless evidence requires revising them.

- Local display codes have no `lf-` prefix and grow beyond six hex
  characters only when necessary to disambiguate stored Tasks.
- Retain the existing Git author slug for `user`; do not introduce
  GitHub-account discovery or new configuration.
- Preserve all existing branch/checkout names and human-assigned
  Session names. Generated Session-name repair runs through owned updates,
  not passive listing. Closed historical Sessions may retain their saved name
  until an authorized update; their displayed Task label is current.
- Read-only inspection on 2026-10-13 found full local IDs in the Task labels
  of Sessions `2030b31b03704d0191762b53b32f22fe` and
  `b039b821023b4144bb1fbc331220c62f`, but not their canonical titles.
  Synthetic stored-title fixtures cover recognized UUID/token repair separately.
- Explicit `--name login-repair` retains that workspace segment and replaces
  the title portion of a new branch (`<author>/<id>-login-repair`).
- Session creation applies the prefix in the SQLite writer after the existing
  purpose formatter. Rename suggestions and repair share that same writer-side
  formatter, including primary and headless Sessions.
