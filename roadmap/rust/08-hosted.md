# 08: Fully Hosted (Phase 3)

Full SaaS control plane. Same infrastructure self-hosters use, but we run it.

## Context

Phase 2 delivers self-hosted with auth and container/K8s execution. The same Helm chart works for:
- Self-hosters running on their own clusters
- Us running for hosted customers

Phase 3 adds the control plane, multi-tenancy, and user-facing features.

## Goal

loopflow.studio becomes a full product:
1. Web UI for wave management
2. Web terminal for Claude login (device flow in browser)
3. Multi-tenant isolation
4. Git provider integration
5. Billing

## Architecture

```
┌────────────────────────────────────────────────────────────────────────────┐
│  loopflow.studio                                                           │
│                                                                            │
│  ┌──────────────────────────────────────────────────────────────────────┐ │
│  │ Control Plane                                                        │ │
│  │                                                                      │ │
│  │  Auth (Clerk) ─── Web UI ─── API ─── Web Terminal                   │ │
│  │                                                                      │ │
│  └──────────────────────────────────────────────────────────────────────┘ │
│                            │                                               │
│                            │ Manages                                       │
│                            ▼                                               │
│  ┌──────────────────────────────────────────────────────────────────────┐ │
│  │ Data Plane (Kubernetes)                                              │ │
│  │                                                                      │ │
│  │  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐         │ │
│  │  │ namespace:     │  │ namespace:     │  │ namespace:     │         │ │
│  │  │ customer-a     │  │ customer-b     │  │ customer-c     │         │ │
│  │  │                │  │                │  │                │         │ │
│  │  │ lfd ─ postgres │  │ lfd ─ postgres │  │ lfd ─ postgres │         │ │
│  │  │ ↓              │  │ ↓              │  │ ↓              │         │ │
│  │  │ agent Jobs     │  │ agent Jobs     │  │ agent Jobs     │         │ │
│  │  │                │  │                │  │                │         │ │
│  │  └────────────────┘  └────────────────┘  └────────────────┘         │ │
│  │                                                                      │ │
│  └──────────────────────────────────────────────────────────────────────┘ │
│                                                                            │
└────────────────────────────────────────────────────────────────────────────┘
```

## Components

### Control Plane API

```typescript
// loopflow-studio/src/api/
├── auth/           # Clerk integration (from Phase 2)
├── waves/          # Wave CRUD, proxies to customer lfd
├── agents/         # Agent status, logs
├── git/            # GitHub/GitLab OAuth
├── terminal/       # Web terminal endpoints
├── billing/        # Stripe integration
└── admin/          # Internal admin
```

### Web UI

```
loopflow-studio/app/
├── dashboard/           # Wave overview
├── waves/
│   ├── [id]/           # Wave detail
│   │   ├── logs/       # Agent logs
│   │   └── settings/   # Wave config
│   └── new/            # Create wave
├── terminal/           # Web terminal
├── settings/
│   ├── claude/         # Claude credential status
│   ├── git/            # GitHub/GitLab connections
│   └── billing/        # Usage and plans
└── admin/              # Internal admin
```

### Multi-Tenancy

#### Namespace Isolation

Each customer gets their own Kubernetes namespace:

```yaml
# Per-customer namespace
apiVersion: v1
kind: Namespace
metadata:
  name: lf-customer-abc123
  labels:
    loopflow.studio/customer: abc123
    loopflow.studio/plan: pro
```

#### Resource Quotas

```yaml
apiVersion: v1
kind: ResourceQuota
metadata:
  name: customer-quota
  namespace: lf-customer-abc123
spec:
  hard:
    requests.cpu: "4"
    requests.memory: 8Gi
    limits.cpu: "8"
    limits.memory: 16Gi
    count/jobs.batch: "10"  # Max concurrent agents
```

#### Network Policies

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: isolate-customer
  namespace: lf-customer-abc123
spec:
  podSelector: {}
  policyTypes:
    - Ingress
    - Egress
  ingress:
    - from:
        - namespaceSelector:
            matchLabels:
              loopflow.studio/component: control-plane
  egress:
    - to:
        - namespaceSelector:
            matchLabels:
              loopflow.studio/customer: abc123
    - to:
        - ipBlock:
            cidr: 0.0.0.0/0  # Allow external (GitHub, etc.)
```

### Web Terminal

For users to run `claude login` in the browser:

```typescript
// loopflow-studio/src/terminal/
import { Terminal } from 'xterm';
import { AttachAddon } from 'xterm-addon-attach';

export function TerminalPage() {
  const [ws, setWs] = useState<WebSocket | null>(null);
  const termRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const terminal = new Terminal();
    terminal.open(termRef.current!);

    // Connect to backend
    const socket = new WebSocket(`wss://loopflow.studio/api/terminal/connect`);
    const attach = new AttachAddon(socket);
    terminal.loadAddon(attach);

    setWs(socket);

    return () => {
      socket.close();
      terminal.dispose();
    };
  }, []);

  return (
    <div>
      <h1>Claude Login</h1>
      <p>Run <code>claude login</code> to authenticate.</p>
      <div ref={termRef} className="terminal" />
    </div>
  );
}
```

Backend spawns isolated container for terminal session:

```typescript
// loopflow-studio/src/terminal/connect.ts
export async function handleTerminalConnect(ws: WebSocket, user: User) {
  // Create terminal container in user's namespace
  const container = await k8s.createPod({
    namespace: `lf-customer-${user.id}`,
    spec: {
      containers: [{
        name: 'terminal',
        image: 'loopflow/terminal:latest',
        command: ['/bin/bash'],
        stdin: true,
        tty: true,
      }],
      restartPolicy: 'Never',
    },
  });

  // Attach to container
  const exec = await k8s.exec(container, {
    stdin: true,
    stdout: true,
    stderr: true,
    tty: true,
  });

  // Proxy WebSocket <-> container
  ws.on('message', (data) => exec.stdin.write(data));
  exec.stdout.on('data', (data) => ws.send(data));

  // Capture credentials when user runs `claude login`
  // Store encrypted in customer's Secret
  watchForCredentials(exec, user);
}
```

### Credential Storage

```typescript
// loopflow-studio/src/credentials/
export async function storeClaudeCredentials(userId: string, credentials: string) {
  // Encrypt with KMS
  const encrypted = await kms.encrypt(credentials);

  // Store as K8s Secret in customer namespace
  await k8s.createOrUpdateSecret({
    namespace: `lf-customer-${userId}`,
    name: 'claude-credentials',
    data: {
      credentials: encrypted,
    },
  });
}
```

### Git Provider OAuth

```typescript
// loopflow-studio/src/git/github.ts
export async function handleGitHubCallback(code: string, user: User) {
  // Exchange code for token
  const token = await github.exchangeCode(code);

  // Store encrypted
  await storeGitCredentials(user.id, 'github', token);

  // List repos user can access
  const repos = await github.listRepos(token);

  return repos;
}

export async function cloneRepo(user: User, repoUrl: string) {
  const token = await getGitCredentials(user.id, 'github');

  // Clone into customer's namespace PVC
  await k8s.createJob({
    namespace: `lf-customer-${user.id}`,
    spec: {
      template: {
        spec: {
          containers: [{
            name: 'clone',
            image: 'loopflow/git-clone:latest',
            env: [{
              name: 'GIT_TOKEN',
              valueFrom: { secretKeyRef: { name: 'git-credentials', key: 'token' } },
            }],
            args: ['clone', repoUrl, '/repos/repo-name'],
            volumeMounts: [{
              name: 'repos',
              mountPath: '/repos',
            }],
          }],
          volumes: [{
            name: 'repos',
            persistentVolumeClaim: { claimName: 'repos' },
          }],
          restartPolicy: 'Never',
        },
      },
    },
  });
}
```

### Billing

```typescript
// loopflow-studio/src/billing/
import Stripe from 'stripe';

const stripe = new Stripe(process.env.STRIPE_SECRET_KEY!);

export async function recordAgentUsage(userId: string, durationSeconds: number) {
  // Find or create subscription item
  const subscription = await getSubscription(userId);

  // Report usage
  await stripe.subscriptionItems.createUsageRecord(
    subscription.itemId,
    {
      quantity: Math.ceil(durationSeconds / 60),  // Round up to minutes
      timestamp: Math.floor(Date.now() / 1000),
      action: 'increment',
    }
  );
}

export const plans = {
  free: {
    agentMinutesPerMonth: 100,
    maxConcurrentAgents: 1,
    price: 0,
  },
  pro: {
    agentMinutesPerMonth: 1000,
    maxConcurrentAgents: 4,
    price: 29,
  },
  team: {
    agentMinutesPerMonth: 10000,
    maxConcurrentAgents: 10,
    price: 99,
  },
};
```

## Provisioning Flow

```
User signs up
    ↓
Create Clerk user
    ↓
Create K8s namespace (lf-customer-{id})
    ↓
Apply ResourceQuota, NetworkPolicy
    ↓
Deploy customer lfd (via Helm)
    ↓
User visits web terminal
    ↓
Run `claude login`
    ↓
Credentials captured, encrypted, stored as Secret
    ↓
User connects GitHub
    ↓
Repos cloned to customer PVC
    ↓
User creates waves
    ↓
Waves run in customer namespace
```

## Done When

- [ ] Web UI shows wave list, status, logs
- [ ] Wave CRUD works through UI
- [ ] Web terminal renders xterm.js
- [ ] `claude login` in terminal captures credentials
- [ ] Credentials stored encrypted in K8s Secret
- [ ] GitHub OAuth connects repos
- [ ] GitLab OAuth connects repos
- [ ] Repos cloned to customer PVC
- [ ] Customer namespaces isolated via NetworkPolicy
- [ ] Resource quotas enforced
- [ ] Stripe integration tracks agent-minutes
- [ ] Free/Pro/Team plans available
- [ ] Usage dashboard shows consumption
- [ ] Upgrade/downgrade flow works

## Future Considerations

- **Multi-region**: Deploy data plane to multiple regions
- **Custom domains**: Let customers use their own domain
- **SSO**: SAML/OIDC for enterprise customers
- **Audit logs**: Track all actions for compliance
- **SLA**: Uptime guarantees for higher tiers

## Dependencies

- Requires: All Phase 1 and Phase 2 work
- This is the end goal - full product
