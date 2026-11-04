# LOO-723 assumptions — 2026-11-04

With no separate design supplied, CLI-only means omit first-time Desktop setup;
an existing release app remains part of every installed update. Explicit raw
promotion may reuse it only when its helper matches the candidate. Acquisition
belongs to `lf install`, not low-level promotion. Existing Sparkle UI remains;
CLI/scheduled installs complete via the receipt owner. These reversible choices
implement Jack Heart's requested coherence and preserve private dev contexts.

The website bootstrap script can select an older published candidate. Headless
implementation assumption (2026-11-04): preserve that published interface by
checking whether recover-switch still requires an id. Older candidates retain
recovery during promotion; candidates containing LOO-723 also recover before
shell destination discovery. Remove this capability check when the bootstrap's
minimum supported release includes optional-id recovery. This does not add a
second recovery owner or infer a new decision by Jack Heart.

Gate assumption (2026-11-04): the interactive concept review left unattended
Desktop restart unresolved. This headless run adopts 1b: defer a new scheduled
update while Desktop is open, without changing the app, CLI or store. Explicit
updates remain immediate; receipt recovery finishes first. This is a reversible
safety choice, not a recorded decision by Jack Heart. Schedules carry their intent
in LF_INSTALL_SCHEDULED and retain their existing cadence when refreshed.
