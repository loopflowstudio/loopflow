The v0.15.3 publisher reached notarization successfully, then failed to verify its crates.io archive: the server schema embeds 68 examples outside the Rust crate. The source archive now carries those examples and compiles on its own.

Move the 85 shared server fixtures into the crate and update Rust, Swift, and documentation references without duplicating data. Release candidates compile the locked source archive before tagging; publication uses the same locked dependencies.

Validation: extracted source archive compiled successfully; 59 Rust server tests, 17 Swift protocol tests, and 54 release automation tests passed. Formatting, Clippy with warnings denied, architecture, and Rust test inventory checks passed. Two existing Rust tests remain ignored.

The existing v0.15.3 GitHub release remains a draft at tag 0766e9cbfd5e01f1c244f0fb3cdecfe8aa6fad9c. This repair does not move that tag, replace its signed artifacts, or claim that publication completed. The release command currently resumes tagged candidates at their original commit; rebuilding that draft from repaired source still needs a recovery path.
