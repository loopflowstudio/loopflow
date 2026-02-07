// Schema versions use commit timestamp + short SHA (e.g. 2026-02-06T12:00:00Z_abcd123).
// This keeps versions monotonic, reproducible, and safe for parallel branches.
pub const SCHEMA_VERSION: &str = "baseline_PENDING";
