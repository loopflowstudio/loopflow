# AWS self-hosted loopflow server

Provision an EC2 Docker host for the legacy self-hosted loopflow cron server.
Container mode and postgres-backed hosting are staging debt scheduled for the M2
substrate cut. Prefer the native Tailscale host in `deploy/PRIVATE_HOST.md` for
maintained SSH-first operations.

```bash
cd deploy/terraform/aws
terraform init
terraform apply \
  -var 'ssh_key_name=your-key' \
  -var 'allowed_ssh_cidrs=["203.0.113.10/32"]' \
  -var 'allowed_https_cidrs=["0.0.0.0/0"]'
```

Terraform creates infrastructure only. Secrets stay out of Terraform state.
After the host boots:

```bash
ssh ubuntu@$(terraform output -raw public_ip)
sudoedit /etc/loopflow-server.env
```

Add:

```bash
DOPPLER_TOKEN=dp.st.x
LF_DOMAIN=lfd.example.com
LF_TLS_MODE=internal
```

Then start:

```bash
sudo systemctl start loopflow-server.service
sudo systemctl status loopflow-server.service
```

Use `allowed_https_cidrs=[]` for Tailscale/private-only hosts where Caddy does not need public 80/443.
