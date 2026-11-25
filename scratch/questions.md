# Assumptions made with nobody here (LOO-1105)

- `ci-fix.md` omits `docs: scripts/testing.md` and points to `scripts/testing.md` on demand (naming the Mac error strings) instead of force-loading ~16.3 KB of Gate + Plan-first prose onto every repair turn.
- Gate's local fmt/clippy runs only when the diff touches Rust; DTO/iOS parity runs when `scripts/test.py --list` selects `ios` or `rust/loopflow-model/` changed.
- `required-integration` stays in `.lf/skills/ci-fix.md` for now so LOO-1105 stays prose-only with no `golden_prompt` churn; moving it into builtin `ci-fix.md` is a separate Task.
- The display-lock preflight checks both `IOConsoleLocked` and `CGSSessionScreenIsLocked` from `ioreg -n Root -d1`; only the unlocked reading (`"IOConsoleLocked" = No`) was observed headlessly.
