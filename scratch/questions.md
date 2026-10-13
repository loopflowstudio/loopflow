# Open assumptions

October 13 — The CI wrapper owns this job's checkout and exact Xcode DerivedData
path. Cleanup identifies newly started compiler processes by their output
arguments under that path, excluding pre-existing PIDs and other output paths.
This is specific to the existing isolated Actions job, not a general process
scheduler. Real Swift/Rust cancellation and a disposable compiler ownership
fixture establish the current cleanup behavior. Hosted toolchain and elapsed
margin remain gate/CI evidence; local improvements do not prove five minutes.
