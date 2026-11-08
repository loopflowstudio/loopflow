# QA assumption

2026-11-08: use a private Machine and an unstarted Session for headless relay
QA. This reaches unavailable, Retry and Close without launching a provider or
touching existing conversations. It leaves live-owner and mounted-pane behavior
unchecked, as recorded in `qa.html`; no product decision is inferred.
