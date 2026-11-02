# Open questions (LOO-708)

No unresolved product choice. Jack Heart's requirement to preserve resume and
exit recording governs the exec cutover; no compatibility break is inferred.
The [working design](loo-708-processes-stay-small.md) records the repaired JSON
mutation paths, settlement semantics, proof limits and remaining acceptance.

Headless assumption retained: a startup failure may use the launcher's actionable
error (OpenCode: "private server failed to start") instead of the deleted connect
poll's "before becoming resumable" wording. Recovery remains required; the
lifecycle fixture asserts the direct error.
