# 03: Container Hardening

Reduce the blast radius of a compromised agent container. Today's containers drop all capabilities and disable privileged mode, but run as root, have no resource limits, and share a repo volume across worktrees.

CIS Docker Benchmark sections 4 (Container Images) and 5 (Container Runtime) provide the framework. Woodpecker CI and Drone CI faced the same class of issues: pipeline containers with too much access escaping to affect other workloads.

## What exists today

Agent containers in `docker.rs` (lines 1511-1518):

```rust
let host_config = HostConfig {
    mounts: Some(mounts),
    network_mode: Some("bridge".to_string()),
    privileged: Some(false),        // good
    cap_drop: Some(vec!["ALL".to_string()]),  // good
    auto_remove: Some(false),
    ..Default::default()            // no resource limits, no security_opt
};
```

Good: `privileged: false`, `cap_drop: ALL`.

Missing:
- Containers run as `root` (`user: Some("root".to_string())`)
- No memory, CPU, or PID limits
- No `no-new-privileges` security option
- Repo volume is shared across all worktrees — a compromised agent in one wave can read/write any other wave's worktree
- No read-only rootfs
- No network egress controls (agents have full outbound access)

The lfd container itself mounts `/var/run/docker.sock` — if compromised, it can create arbitrary containers on the host.

## What exists after this

Agent containers run as a non-root user with memory/CPU/PID limits, `no-new-privileges`, and scoped volume mounts. The lfd container accesses Docker through a socket proxy that restricts API operations.

## What we learned from phases 01 and 02

- Path/ID sanitization is now centralized in `lfd::security`, so this phase can stay focused on runtime isolation instead of input validation.
- Worktree names and repo paths are now normalized/canonicalized before use, which lowers risk when we evaluate per-worktree mounts.
- Auth is already split into read vs mutate tiers, so this phase does not need extra auth-specific branching.

## Security boundary for this phase

This phase reduces impact when an agent runtime is compromised:

- A compromised container has fewer kernel/runtime privileges.
- Resource limits reduce single-run host exhaustion.
- Runtime defaults make accidental container-escape misconfiguration less likely.

This phase does not provide:

- A guarantee against all container escape techniques.
- Protection against full host compromise.
- Full cross-worktree confidentiality in the current shared-volume model (addressed incrementally).

## Implementation

### Non-root agent user

Update `docker/agent/Dockerfile` to create a non-root user:

```dockerfile
RUN groupadd -r agent && useradd -r -g agent -m agent
USER agent
```

Update `docker.rs` to use `user: Some("agent".to_string())` instead of `root`. Verify credential mounts are readable by the agent user (group-readable or world-readable in the mounted source).

### Resource limits

Add to the `HostConfig` in `docker.rs`:

```rust
memory: Some(4 * 1024 * 1024 * 1024),  // 4 GB
memory_swap: Some(4 * 1024 * 1024 * 1024),  // no swap beyond memory
cpu_quota: Some(200_000),  // 2 CPUs (100_000 per CPU)
pids_limit: Some(512),
```

Make these configurable via `lfd.yaml` under `executor.limits.*` with sensible defaults. Agent workloads (Claude Code, Codex) are memory-hungry — 4 GB is a reasonable default, but operators should be able to raise it.

### Security options

```rust
security_opt: Some(vec!["no-new-privileges:true".to_string()]),
```

This prevents `setuid`/`setgid` binaries from escalating privileges inside the container. Standard CIS Docker recommendation.

### Read-only rootfs (optional, investigate)

```rust
read_only_rootfs: Some(true),
```

With explicit tmpfs mounts for paths agents need to write:

```rust
tmpfs: Some(HashMap::from([
    ("/tmp".to_string(), "size=512m".to_string()),
    ("/home/agent".to_string(), "size=256m".to_string()),
])),
```

This needs testing — Claude Code, Codex, and other agents may write to unexpected paths. Start with investigation, ship if feasible.

### Docker socket proxy

Replace direct socket mount with [Tecnativa/docker-socket-proxy](https://github.com/Tecnativa/docker-socket-proxy) in `docker-compose.yml`:

```yaml
docker-socket-proxy:
  image: tecnativa/docker-socket-proxy
  environment:
    CONTAINERS: 1
    IMAGES: 1
    VOLUMES: 1
    POST: 1
    NETWORKS: 0
    SERVICES: 0
    SWARM: 0
    SECRETS: 0
    EXEC: 0
  volumes:
    - /var/run/docker.sock:/var/run/docker.sock:ro
  restart: unless-stopped

lfd:
  environment:
    DOCKER_HOST: tcp://docker-socket-proxy:2375
  # Remove: /var/run/docker.sock mount
```

This restricts lfd to only the Docker API operations it needs (containers, images, volumes) and blocks dangerous operations (exec into containers, network creation, swarm management, secrets access). Bollard supports TCP connections via `Docker::connect_with_http`.

For native mode (no Docker Compose), lfd continues using the Unix socket directly. The proxy is a container-mode hardening.

### Network isolation

Add explicit Docker networks to `docker-compose.yml`:

```yaml
networks:
  frontend:    # Caddy <-> lfd
  backend:     # lfd <-> postgres, lfd <-> docker-socket-proxy
  # Agent containers: created by lfd on a separate bridge
```

Agent containers should not be able to reach Postgres or the socket proxy. lfd creates them on an isolated network (or the default bridge, which is separate from the Compose networks).

### Cross-worktree isolation

The current repo volume model mounts the entire volume (containing all worktrees) into each agent container. A compromised agent can `cd ..` to access sibling worktrees.

Options (in order of preference):
1. **Bind-mount the specific worktree** instead of the entire volume. This requires the worktree path to exist on the host or in a volume that lfd can sub-mount. Needs investigation into Bollard's bind mount capabilities for volume subdirectories.
2. **Use `--workdir` enforcement** in the container and rely on the non-root user not having write access to sibling worktree directories. Weaker — agents can still read.
3. **Accept the risk** and document it. Worktrees within a single repo are already branches of the same codebase. Cross-worktree access is a confidentiality issue, not an integrity issue (agents can already push to any branch via git).

Checkpoint expectation: ship options 1-2 only if they are reliable in this phase; otherwise document option 3 explicitly and carry sub-mount isolation into a follow-up.

## Verification

- Agent containers run as non-root: `docker inspect <container> | jq '.[0].Config.User'`
- Resource limits applied: `docker stats` shows memory cap
- Socket proxy blocks exec: `docker exec <agent-container> sh` fails from lfd container
- No-new-privileges: `docker inspect <container> | jq '.[0].HostConfig.SecurityOpt'`

## Open questions

- Are `4 GB / 2 CPU / 512 PIDs` safe defaults for both Claude Code and Codex under real prompts?
- Can Bollard mount only a worktree subpath from the repo volume in a portable way, or does that require host bind mounts?

## Checkpoints

1. Non-root user + resource limits + `no-new-privileges` land with regression tests.
2. Compose mode uses a socket proxy and isolated networks.
3. Cross-worktree decision is explicit: ship scoped mounts, or document risk and defer with a tracked follow-up.

## Try it

- Run two heavy wave executions concurrently and confirm one container hitting limits does not destabilize lfd.
- From an agent shell, attempt to read a sibling worktree path and verify the documented isolation behavior.

## What might change

- Read-only rootfs may move to a later phase if agent CLIs require writes outside known temp/home paths.
- If per-worktree mounts are not technically reliable yet, this phase will ship hard runtime limits first and defer mount isolation as a smaller dedicated item.
