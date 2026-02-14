# 04: EC2 Infrastructure

Deploy the containerized lfd stack (Phases 01–02) to an EC2 instance. Terraform in the studio repo.

## What exists after this

An EC2 instance running `docker compose up` with lfd + postgres + agent containers, TLS-terminated via Caddy. Accessible via SSH. Same compose file tested locally now runs remotely.

## Why this is simple

Phases 01–03 solved everything: lfd, agents, postgres, credential mounting, repo volumes, auth — all in Docker. EC2 just needs Docker installed, the compose file copied over, and a `.env` with the auth token. Auto-migration handles postgres schema on first start.

## Terraform (../studio)

```hcl
# studio/terraform/dev/main.tf

resource "aws_instance" "lfd" {
  ami           = data.aws_ami.ubuntu.id   # Ubuntu 24.04 ARM
  instance_type = "t4g.medium"             # 2 vCPU, 4GB RAM, ARM, ~$25/mo
  key_name      = var.ssh_key_name

  vpc_security_group_ids = [aws_security_group.lfd.id]

  root_block_device {
    volume_size = 50  # GB, repos + docker images + worktrees
    volume_type = "gp3"
  }

  user_data = file("${path.module}/setup.sh")

  tags = {
    Name = "lfd-dev"
  }
}

resource "aws_security_group" "lfd" {
  name = "lfd-dev"

  ingress {
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = [var.my_ip_cidr]  # SSH from your IP only
  }

  ingress {
    from_port   = 443
    to_port     = 443
    protocol    = "tcp"
    cidr_blocks = [var.my_ip_cidr]  # HTTPS from your IP only
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

resource "aws_eip" "lfd" {
  instance = aws_instance.lfd.id
}

output "public_ip" {
  value = aws_eip.lfd.public_ip
}
```

### Setup script

```bash
#!/bin/bash
# studio/terraform/dev/setup.sh
# Runs as root on first boot via user_data

set -euo pipefail

# Install Docker
apt-get update
apt-get install -y docker.io docker-compose-v2
systemctl enable --now docker

# Create lfd user with docker access (for SSH, not for the container — compose runs as root)
useradd -m -s /bin/bash -G docker lfd
```

### TLS via Caddy

Add Caddy as a reverse proxy for TLS termination. Caddy generates a self-signed cert automatically for IP addresses (no domain required).

```yaml
# Add to docker-compose.yml for remote deployment
caddy:
  image: caddy:2-alpine
  ports:
    - "443:443"
  volumes:
    - ./Caddyfile:/etc/caddy/Caddyfile
    - caddy-data:/data
  depends_on:
    - lfd
```

```
# Caddyfile
:443 {
  tls internal
  reverse_proxy lfd:2486
}
```

`tls internal` tells Caddy to generate a self-signed cert. Concerto trusts it by pinning the cert fingerprint on first connect.

### Semi-manual steps after terraform apply

```bash
# Copy compose file, Caddyfile, and .env to the instance
# .env needs: LFD_AUTH_PROVIDER=static, LFD_AUTH_TOKEN, ANTHROPIC_API_KEY, GH_TOKEN
# See .env.example for full list
scp docker-compose.yml Caddyfile .env lfd-dev:~/

# Copy credential mounts if using subscriptions (uncomment volumes in compose)
scp -r ~/.claude lfd-dev:~/.claude
scp ~/.gitconfig lfd-dev:~/.gitconfig

# Start the stack (postgres auto-migrates on first start)
ssh lfd-dev 'docker compose up -d'

# Verify
curl -k https://lfd-dev/health

# Add a repo
curl -k -X POST https://lfd-dev/v0/repos \
  -H "Authorization: Bearer $LFD_TOKEN" \
  -d '{"url": "https://github.com/you/repo.git"}'
```

### SSH config (local machine)

```
# ~/.ssh/config
Host lfd-dev
  HostName <elastic-ip>
  User lfd
  IdentityFile ~/.ssh/your-key
```

## Constraints

- **ARM (t4g)**: Need ARM Docker images. The Dockerfile uses `rust:1.82-bookworm` and `debian:bookworm-slim` which are multi-arch, so building on the ARM instance should work. Cross-compilation from Mac (Apple Silicon → ARM Linux) avoids the slow on-instance build but adds toolchain complexity. Simplest path: build on the instance.
- **Single instance**: No load balancer, no HA. Dev box.
- **IP-restricted**: Security group only allows your IP.
- **Self-signed TLS**: Caddy generates internal certs. Concerto pins the cert fingerprint on first connect.
- **Cargo build time**: Clean `docker build` takes 5-10 minutes. No `cargo-chef` caching yet. Acceptable for infrequent deploys.

## Done when

- `terraform apply` creates the instance with Docker installed
- `docker compose up -d` starts lfd + postgres
- `curl -k https://lfd-dev/health` returns OK (TLS via Caddy)
- A wave can run remotely (agent in container on EC2)
- Logs stream back to Concerto over HTTP
