# LOO-723 assumptions — 2026-11-04

With no separate design supplied, CLI-only means omit first-time Desktop setup;
an existing release app remains part of every installed update. Explicit raw
promotion may reuse it only when its helper matches the candidate. Acquisition
belongs to `lf install`, not low-level promotion. Existing Sparkle UI remains;
CLI/scheduled installs complete via the receipt owner. These reversible choices
implement Jack Heart's requested coherence and preserve private dev contexts.
