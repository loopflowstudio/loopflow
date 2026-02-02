# 07: Container and Kubernetes Deployment

Production deployment options for self-hosted lfd.

## Context

With auth (05) and executors (06), lfd can run remotely with proper security and isolation.

Now we need packaging for easy deployment.

## Goal

1. Docker Compose for simple self-hosted deployment
2. Helm chart for Kubernetes deployment
3. Container images published to registry
4. Documentation for both paths

## Container Images

### loopflow/lfd

The daemon image:

```dockerfile
# rust/lfd/Dockerfile
FROM rust:1.93-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY rust ./rust
COPY proto ./proto

RUN cargo build -p lfd --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m -s /bin/bash lfd
USER lfd

WORKDIR /app
COPY --from=builder /app/target/release/lfd /usr/local/bin/lfd

EXPOSE 50051 8080

ENTRYPOINT ["lfd", "run"]
```

### loopflow/agent

The agent execution environment:

```dockerfile
# images/agent/Dockerfile
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    git \
    openssh-client \
    && rm -rf /var/lib/apt/lists/*

# Install Claude CLI
RUN curl -fsSL https://claude.ai/install.sh | sh

# Install other agent CLIs as needed
# RUN npm install -g @anthropic/codex
# RUN curl -fsSL https://gemini.google.com/cli/install.sh | sh

# Non-root user
RUN useradd -m -s /bin/bash agent
USER agent
WORKDIR /workspace

# Default to claude
ENTRYPOINT ["claude"]
```

### Build and Push

```yaml
# .github/workflows/docker.yml
name: Docker Images

on:
  push:
    tags: ['v*']
  workflow_dispatch:

jobs:
  build-lfd:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Login to Docker Hub
        uses: docker/login-action@v3
        with:
          username: ${{ secrets.DOCKERHUB_USERNAME }}
          password: ${{ secrets.DOCKERHUB_TOKEN }}

      - name: Build and push lfd
        uses: docker/build-push-action@v5
        with:
          context: .
          file: rust/lfd/Dockerfile
          push: true
          tags: |
            loopflow/lfd:latest
            loopflow/lfd:${{ github.ref_name }}
          platforms: linux/amd64,linux/arm64

  build-agent:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Build and push agent
        uses: docker/build-push-action@v5
        with:
          context: images/agent
          push: true
          tags: |
            loopflow/agent:latest
            loopflow/agent:${{ github.ref_name }}
          platforms: linux/amd64,linux/arm64
```

## Docker Compose (Self-Hosted)

```yaml
# deploy/docker-compose/docker-compose.yml
services:
  traefik:
    image: traefik:v3.0
    command:
      - "--api.insecure=true"
      - "--providers.docker=true"
      - "--providers.docker.exposedbydefault=false"
      - "--entrypoints.websecure.address=:443"
      - "--certificatesresolvers.letsencrypt.acme.httpchallenge=true"
      - "--certificatesresolvers.letsencrypt.acme.httpchallenge.entrypoint=web"
      - "--certificatesresolvers.letsencrypt.acme.email=${ACME_EMAIL}"
      - "--certificatesresolvers.letsencrypt.acme.storage=/certs/acme.json"
      - "--entrypoints.web.address=:80"
      - "--entrypoints.web.http.redirections.entrypoint.to=websecure"
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - traefik-certs:/certs
    networks:
      - loopflow

  lfd:
    image: loopflow/lfd:latest
    environment:
      LFD_STORAGE: postgres
      LFD_DATABASE_URL: postgres://lfd:${POSTGRES_PASSWORD}@postgres:5432/lfd
      LFD_AUTH_PROVIDER: loopflow.studio
      LFD_AUTH_ALLOWED_USERS: ${ALLOWED_USERS}
      LFD_EXECUTOR_TYPE: container
      LFD_EXECUTOR_IMAGE: loopflow/agent:latest
      LFD_GRPC_ADDR: 0.0.0.0:50051
      LFD_HTTP_ADDR: 0.0.0.0:8080
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - ${CLAUDE_CREDENTIALS:-~/.claude}:/claude-credentials:ro
      - repos:/repos
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.lfd-grpc.rule=Host(`${LFD_HOSTNAME}`)"
      - "traefik.http.routers.lfd-grpc.entrypoints=websecure"
      - "traefik.http.routers.lfd-grpc.tls.certresolver=letsencrypt"
      - "traefik.http.routers.lfd-grpc.service=lfd-grpc"
      - "traefik.http.services.lfd-grpc.loadbalancer.server.port=50051"
      - "traefik.http.services.lfd-grpc.loadbalancer.server.scheme=h2c"
      - "traefik.http.routers.lfd-http.rule=Host(`${LFD_HOSTNAME}`) && PathPrefix(`/health`, `/status`, `/metrics`)"
      - "traefik.http.routers.lfd-http.service=lfd-http"
      - "traefik.http.services.lfd-http.loadbalancer.server.port=8080"
    depends_on:
      postgres:
        condition: service_healthy
    networks:
      - loopflow

  postgres:
    image: postgres:16
    environment:
      POSTGRES_USER: lfd
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
      POSTGRES_DB: lfd
    volumes:
      - postgres-data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U lfd"]
      interval: 5s
      timeout: 5s
      retries: 12
    networks:
      - loopflow

networks:
  loopflow:
    name: loopflow

volumes:
  traefik-certs:
  postgres-data:
  repos:
```

```bash
# deploy/docker-compose/.env.example
LFD_HOSTNAME=lfd.example.com
ACME_EMAIL=admin@example.com
POSTGRES_PASSWORD=changeme
ALLOWED_USERS=user_abc123,user_def456
CLAUDE_CREDENTIALS=/home/user/.claude
```

### Usage

```bash
cd deploy/docker-compose

# Configure
cp .env.example .env
# Edit .env with your settings

# Start
docker compose up -d

# Check status
docker compose ps
docker compose logs -f lfd

# Connect
lf auth login
lf --server lfd.example.com wave list
```

## Helm Chart

```
deploy/helm/loopflow/
├── Chart.yaml
├── values.yaml
├── templates/
│   ├── _helpers.tpl
│   ├── deployment-lfd.yaml
│   ├── service-lfd.yaml
│   ├── statefulset-postgres.yaml
│   ├── service-postgres.yaml
│   ├── ingress.yaml
│   ├── secret-postgres.yaml
│   ├── secret-claude.yaml
│   ├── configmap-lfd.yaml
│   ├── pvc-repos.yaml
│   ├── serviceaccount.yaml
│   ├── role.yaml
│   ├── rolebinding.yaml
│   └── NOTES.txt
```

### Chart.yaml

```yaml
apiVersion: v2
name: loopflow
description: Loopflow daemon for wave orchestration
type: application
version: 0.1.0
appVersion: "0.8.0"
```

### values.yaml

```yaml
lfd:
  image:
    repository: loopflow/lfd
    tag: ""  # Defaults to appVersion
    pullPolicy: IfNotPresent

  replicas: 1

  auth:
    provider: loopflow.studio
    allowedUsers: []
    # - user_abc123
    # - user@example.com

  executor:
    type: kubernetes
    image: loopflow/agent:latest
    serviceAccount: loopflow-agent

  resources:
    requests:
      memory: 256Mi
      cpu: 100m
    limits:
      memory: 1Gi
      cpu: 1000m

  service:
    type: ClusterIP
    grpcPort: 50051
    httpPort: 8080

postgres:
  enabled: true
  auth:
    postgresPassword: ""  # Generated if empty
    database: lfd
  persistence:
    enabled: true
    size: 10Gi
    storageClass: ""

ingress:
  enabled: true
  className: nginx
  annotations:
    cert-manager.io/cluster-issuer: letsencrypt-prod
    nginx.ingress.kubernetes.io/backend-protocol: GRPC
  hosts:
    - host: lfd.example.com
      paths:
        - path: /
          pathType: Prefix
  tls:
    - secretName: lfd-tls
      hosts:
        - lfd.example.com

claude:
  existingSecret: ""
  # Or create secret from credentials
  credentials: ""  # Base64 encoded

repos:
  persistence:
    enabled: true
    size: 50Gi
    storageClass: ""

serviceAccount:
  create: true
  name: ""

rbac:
  create: true
```

### deployment-lfd.yaml

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: {{ include "loopflow.fullname" . }}-lfd
  labels:
    {{- include "loopflow.labels" . | nindent 4 }}
    app.kubernetes.io/component: lfd
spec:
  replicas: {{ .Values.lfd.replicas }}
  selector:
    matchLabels:
      {{- include "loopflow.selectorLabels" . | nindent 6 }}
      app.kubernetes.io/component: lfd
  template:
    metadata:
      labels:
        {{- include "loopflow.selectorLabels" . | nindent 8 }}
        app.kubernetes.io/component: lfd
    spec:
      serviceAccountName: {{ include "loopflow.serviceAccountName" . }}
      containers:
        - name: lfd
          image: "{{ .Values.lfd.image.repository }}:{{ .Values.lfd.image.tag | default .Chart.AppVersion }}"
          imagePullPolicy: {{ .Values.lfd.image.pullPolicy }}
          ports:
            - name: grpc
              containerPort: 50051
              protocol: TCP
            - name: http
              containerPort: 8080
              protocol: TCP
          env:
            - name: LFD_STORAGE
              value: postgres
            - name: LFD_DATABASE_URL
              valueFrom:
                secretKeyRef:
                  name: {{ include "loopflow.fullname" . }}-postgres
                  key: url
            - name: LFD_AUTH_PROVIDER
              value: {{ .Values.lfd.auth.provider }}
            - name: LFD_AUTH_ALLOWED_USERS
              value: {{ .Values.lfd.auth.allowedUsers | join "," | quote }}
            - name: LFD_EXECUTOR_TYPE
              value: {{ .Values.lfd.executor.type }}
            - name: LFD_EXECUTOR_IMAGE
              value: {{ .Values.lfd.executor.image }}
            - name: LFD_EXECUTOR_NAMESPACE
              valueFrom:
                fieldRef:
                  fieldPath: metadata.namespace
            - name: LFD_EXECUTOR_CLAUDE_SECRET
              value: {{ include "loopflow.fullname" . }}-claude
            - name: LFD_GRPC_ADDR
              value: "0.0.0.0:50051"
            - name: LFD_HTTP_ADDR
              value: "0.0.0.0:8080"
          livenessProbe:
            httpGet:
              path: /health
              port: http
            initialDelaySeconds: 10
            periodSeconds: 10
          readinessProbe:
            httpGet:
              path: /health
              port: http
            initialDelaySeconds: 5
            periodSeconds: 5
          resources:
            {{- toYaml .Values.lfd.resources | nindent 12 }}
          volumeMounts:
            - name: repos
              mountPath: /repos
      volumes:
        - name: repos
          {{- if .Values.repos.persistence.enabled }}
          persistentVolumeClaim:
            claimName: {{ include "loopflow.fullname" . }}-repos
          {{- else }}
          emptyDir: {}
          {{- end }}
```

### role.yaml (RBAC for creating Jobs)

```yaml
{{- if .Values.rbac.create }}
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: {{ include "loopflow.fullname" . }}-agent-manager
  labels:
    {{- include "loopflow.labels" . | nindent 4 }}
rules:
  - apiGroups: ["batch"]
    resources: ["jobs"]
    verbs: ["create", "get", "list", "watch", "delete"]
  - apiGroups: [""]
    resources: ["pods", "pods/log"]
    verbs: ["get", "list", "watch"]
{{- end }}
```

### Usage

```bash
# Add repo
helm repo add loopflow https://loopflowstudio.github.io/charts
helm repo update

# Create namespace
kubectl create namespace loopflow

# Create Claude credentials secret
kubectl create secret generic claude-credentials \
  --from-file=credentials=${HOME}/.claude \
  -n loopflow

# Install
helm install loopflow loopflow/loopflow \
  --namespace loopflow \
  --set lfd.auth.allowedUsers="{user_abc123}" \
  --set ingress.hosts[0].host=lfd.example.com \
  --set claude.existingSecret=claude-credentials

# Check status
kubectl get pods -n loopflow
kubectl logs -f deployment/loopflow-lfd -n loopflow

# Connect
lf auth login
lf --server lfd.example.com wave list
```

## Done When

- [ ] `loopflow/lfd` image builds and runs
- [ ] `loopflow/agent` image builds with Claude CLI
- [ ] Images published to Docker Hub on release
- [ ] Docker Compose deploys lfd + postgres + traefik
- [ ] Docker Compose handles TLS via Let's Encrypt
- [ ] Helm chart deploys to Kubernetes
- [ ] Helm chart creates RBAC for Job creation
- [ ] Helm chart handles ingress with TLS
- [ ] Documentation for both deployment paths
- [ ] Health checks work through load balancer

## Dependencies

- Requires: 05-auth, 06-executors
- Enables: Self-hosted remote access, path to 08-hosted
