# Task 3fcb3f gate notes

- Assumption: `provider_account::` stays outside this gate's filter. The diff changes only the cutover export. Run from this checkout, 13 `account_first_tests` fail on Jack Heart's Mac with `account reference manabot-eng has no resident login`; the same binary run outside any repository passes 32/32.
- Cause: `shared_home` fixtures set `LF_HOME` but resolve the cwd's repository, so the Repo layer reads the main checkout's `lf.config.local.yaml` (`account.codex: [manabot-eng]`). CI has no such file. Fixture isolation is separate work.
- Open: that bare id looks like a repo-scope cutover export the hand repair missed; not confirmed. It still selects on the real store through the email prefix match. Left unedited: it is outside this checkout.
