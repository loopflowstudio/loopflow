# Open questions / follow-ups

- Keep image builds as `docker build` CLI dependency, or move builds to Docker API/BuildKit to remove CLI runtime requirements?
- Should CLI fork execution also support `fork.select: one/prompt`, or is `select: all` sufficient for now?
