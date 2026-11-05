# 998c9b demo: upgrade docs and help

Reviewed 2026-11-05, headless; an agent stood in for Jack Heart. No person saw this.
Evidence: branch `jack-heart/998c9b-upgrade-docs-and` at 94826fd632, PR #1826.

Experience reviewed: a reader runs `lf help monitor rollover`, `lf help schedule`
and `lf help self install rollback` from the branch build, and reads `docs/ops.md`
"Updates", `docs/architecture/machines.md` and `release/README.md`.

Observed:

- `lf help monitor rollover` names live turn, running Graph, headless Run, saved
  history and unsent draft, and says viewers don't hold a Session. Matches
  `owner_disposition` (`rollover.rs:393`) and `owner_retention` (`sessions.rs:676`).
- It says a move ends the owner and agent, then starts `session resume` fresh.
- `lf help schedule` resolves to `lf self install schedule`; `published.rs:854`
  sets RunAtLoad and `:740` the crontab marker `docs/ops.md` tells readers to delete.
- Rollback help says it refuses receipt-managed installs (`install.rs:3004`).
- The fallback sentence matches `install.rs:2828` and `:2897`.
- `release/install.sh:303` warns and `published.rs` bails on a failed bootstrap,
  as `docs/ops.md` says.

- `lf monitor rollover --dry-run` on the installed 0.15.1 listed real owners it
  would move and stopped nothing. Only the first 20 lines were read.

Checks: `cargo fmt --check` passes; `cargo test --test integration documented_commands::`
4 passed; `cargo test --lib rollover` 4 passed.

Not exercised: `launchctl disable`/`bootout` on the real `com.loopflow.refresh`.
It would change this Machine's updates; an earlier pass checked bootstrap's exit 5
with a throwaway label. Hosted CI skipped its Rust jobs on the PR's last run.

Design changes: none. Open product follow-ups stay in `scratch/questions.md`.

Next: deliver PR #1826. Jack Heart's comment `f8f13216` asks to ship the
improvements from the 2026-11-05 audits.
