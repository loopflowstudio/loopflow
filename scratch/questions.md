# 998c9b open questions

Done: the three audited promises and the `lf self install`/`lf monitor` help now
match `install.rs`, `published.rs`, `rollover.rs` and `lifetime.rs`; every claim in
the diff was reread against its code on 2026-11-05. No code change. Remaining:
deliver the PR.

Docs now describe these behaviors; the Task forbids changing them. Jack Heart may want follow-ups.

- `lf schedule` and `lf install --dev` fail on a disabled `com.loopflow.refresh` (`launchctl bootstrap` exits 5, checked with a throwaway label on macOS 26). Release installs only warn (`release/install.sh:303`). Treating a disabled job as a choice would remove the error.
- Linux has no lasting off switch: `cron_table` restores the line on each install.
- No install compares versions, so a Machine ahead of its stream can move back on a scheduled run (`install_release`, `published.rs:102`).
