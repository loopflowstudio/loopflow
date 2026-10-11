# Assumptions

2026-10-11: Reuse a locally available image, including an explicit --image. Fresh
hosted runners still fetch stable Rust; developers refresh mutable tags with
docker pull. This avoids recontacting ECR for the second installation proof.
Bounded retries address transient throttling; a persistent registry outage must
still fail visibly rather than skip installation verification.
