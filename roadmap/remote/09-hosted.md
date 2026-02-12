# 09: Hosted SaaS

loopflow.studio runs lfd for you. Same infrastructure self-hosters use, but we operate it.

## What exists after this

Users sign up at loopflow.studio, connect their GitHub, and waves run in managed infrastructure. No EC2, no SSH, no sysadmin. Concerto connects via the same HTTP+WS protocol.

## Simple version (start here)

Same Docker Compose stack from Phase 02, running on studio's infrastructure. One EC2 per customer (or shared, manually assigned). No new executor backend — lfd creates agent containers via Docker socket, same as always.

Provisioning:
1. Sign up at loopflow.studio, connect GitHub
2. Studio provisions a Docker Compose instance (lfd + postgres)
3. Concerto connects via Phase 07 discovery — sign in, auto-discover, done

## Scaling version (when needed)

K8s cluster for tenant isolation and placement. Each customer gets a namespace. lfd runs as a pod with Docker socket access on its node, creates agent containers locally via Docker API — same execution model as Phase 02, no K8s executor.

```
K8s cluster
  ├── customer-1 namespace
  │     ├── lfd pod (Docker socket mounted)
  │     │     └── creates agent containers on same node
  │     └── postgres pod
  │
  └── customer-2 namespace
        ├── lfd pod (Docker socket mounted)
        │     └── creates agent containers on same node
        └── postgres pod
```

K8s handles:
- **Tenant isolation** — namespaces, NetworkPolicy, ResourceQuota
- **Placement** — which node runs which customer's lfd
- **Lifecycle** — restarts, health checks, rolling updates

Docker handles:
- **Agent execution** — lfd creates containers via Docker socket, same as Phase 02
- **Repo volumes** — local to the node, shared between lfd and agents

No shared filesystem problem — lfd and agents are always co-located on the same node.

## What's needed

### Simple version
- Provisioning API in studio (create instance, configure, return connection info)
- Studio serves auth (Phase 07, already built by then)

### Scaling version (later)
- EKS cluster
- Helm charts for lfd + postgres per namespace
- Provisioning API creates namespaces instead of instances
- DaemonSet or node config to ensure Docker socket available to lfd pods

## Done when

- User can sign up and connect GitHub
- Instance provisioned automatically
- Waves run in managed infrastructure
- Same Concerto app works without configuration changes
