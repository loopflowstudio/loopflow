# Open questions

## Branch follow-ups

- Should Docker image builds continue to require `docker build` CLI at runtime, or move to Docker API/BuildKit?
- Should CLI fork execution add support for `fork.select: one` and `fork.select: prompt`, or keep `select: all` only for now?
