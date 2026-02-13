# Open questions / follow-ups

- `LfdConfig::load()` still swallows YAML parse errors and falls back to defaults. That means invalid credential mount syntax in `~/.lf/lfd.yaml` may not surface as a startup parse error unless config loading behavior is tightened.
- Fork-all tests in Docker mode no longer hard-fail with the old explicit rejection, but test runs still emit `git` "cannot change to ...-fork-*" stderr during cleanup/prompt building. Behavior should be validated in a real Docker-backed fork flow (`lf flow roadmap-reduce` with `executor.type: docker`).
- Image pipeline currently shells out to `docker build` CLI instead of using the Docker API directly. This assumes the Docker CLI binary is installed alongside a reachable daemon.
