# CI repair

- Failed head `0a0753d16dc5a72b4b5e6cbca405edd483393988`, run `43532011181`, library partition 4/5: continuation request test returned "cannot resolve an executable path for `lf`".
- Main `39b37794274138845b6a98c5d6d90b7cd138f725`: CI run `43528364053` passed; `lf sync` made no changes.
- Original compiled continuation test with `LF_*` and `CARGO_BIN_EXE_lf` unset, `PATH=/usr/bin:/bin`: reproduced the same failure.
- Rebuilt continuation test in that environment: passed, 0.07 s.
- `cargo fmt --all -- --check`, `git diff --check`: passed.
- Library test build passed with a macOS linker warning about the large `__eh_frame` section.
- `cargo clippy --all-targets -- -D warnings`: passed.
