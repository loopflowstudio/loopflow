# Assumptions made with nobody here (LOO-1105)

- Gate's local fmt/clippy runs only when the diff touches Rust; the brief says "require" without a condition.
- The focused-test commands leave `scripts/testing.md` when they move into `implement.md`, so gate and ci-fix read them from `AGENTS.md`.
- The display-lock preflight uses `IOConsoleLocked` from `ioreg`; only the unlocked reading (`<false/>`) was seen.
