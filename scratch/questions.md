# Open questions (LOO-885)

- **Gate with no PR creates one.** Assumed: `lf pr publish` from gate creates a
  ready PR when the branch has none (`queue` graph, bare `lf gate`), because CI
  is the test driver and a ready PR is not armed. The other option is to treat
  "no PR" like "no CI" and stay local unless the caller asked to publish.
- **Gate does not wait for CI to finish.** Assumed: it reports pending by head
  and `land --wait-and-fix` waits. A bare `lf gate` then ends without a CI
  verdict when the matrix (14–15 min here) outlasts the review.
- **`.lf/graphs/pursue-auto.yaml`** ends in `cmd: task pr land` (unknown
  command) and loops implement after gate, which would now run with scratch
  cleared. Left alone; delete or repair is a separate call.
