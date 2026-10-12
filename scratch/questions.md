# Assumptions

2026-10-11: Jack Heart's comment `2d140be4-23a0-452c-ac69-5bd87b41d3e3` supersedes hourly inventory and 15-second comment polling. Ten-second freshness applies to attempts, while the successful watermark advances only after complete ingestion. Failed or interrupted queries retain the watermark. Pagination may require more than one HTTP request for a large delta; ordinary empty/single-page checks use one team query.

Live measurement gap: Doppler lists LINEAR_CLIENT_ID and LINEAR_CLIENT_SECRET, but no access token/API key. No auth repair or production sync reactivation was attempted. Live before/after counts and rejected-request quota accounting remain unproved. Linear documents HTTP 400 RATELIMITED and millisecond resets but not rejected-request accounting: https://linear.app/developers/rate-limiting .
