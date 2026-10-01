# Implementation decisions

- 2026-09-30: The task names the token increase but also requires only outliers
  to be excerpted. Product memory is 66,112 bytes, above the old 65,536-byte cap.
  Raise the memory byte default to 128 KiB, matching the other 16,000-token
  source budgets. Total input remains 64,000 tokens / 512 KiB; explicit
  configuration overrides remain authoritative.
