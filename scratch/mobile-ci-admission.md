# Admission-test timeout on mobile publication

CI42938518455 on3f1a7d4347 passed iOS and all jobs except Rust library4/5. Its17-case admission migration test exceeded120.012s;518 other tests passed. No assertion failure or stack was supplied. Same-source local reproduction passed23.73s. A second profiled run passed27.72s; the sample captured only three stacks in historical schema construction, too few for a time distribution. The first sampling attempt missed the already-exited process.

The test builds an identical pre-planning_peers schema17 times, then seeds and upgrades each variant. Construct the starting schema once and use SQLite backup into each independent in-memory connection. Every case still receives its own released data, runs the real upgrade, and checks unchanged saved Work, admission and retained observation evidence. No production code or timeout change. The focused candidate run is live in4680, /tmp/mobile-admission-ci-fixed.log. Rust compiler links a >16MB unwind section and warns about exception performance; this is not the CI timeout diagnosis.

Logs: /Users/jack/.lf-machine/reviews/mobile-718-20261110/desktop-scroll-final/{ci-failure.log,admission-profiled-repro.log,admission-repro.sample.txt}; baseline /tmp/mobile-admission-ci-repro.log. No CI retry or cancellation.

Candidate passed17.49s versus baseline23.73s (one local comparison; no Linux or tail-latency claim). All17 data variants still pass. cargo fmt --check passed. All-target Clippy is live in53433, /tmp/mobile-admission-ci-clippy.log.

All-target Clippy passed68s; fmtcheck and whitespace passed. No runtime change. iPhone Mirroring reports “iPhone in Use”; no physical scroll trial performed. The signed84bbc89cb phone app stays installed. Sync/publication proceeding without auto-merge.
