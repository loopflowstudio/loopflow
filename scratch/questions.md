# LOO-624 open questions and assumptions

- **Images (assumed):** RunPod and Fly boot stock `ubuntu:24.04` and install
  TOOLS at boot instead of a prebuilt registry image, so no registry or image
  build is needed yet. Cost: minutes per boot (SSH_WAIT raised to 20 min). A
  pushed image is the follow-up once a live run shows the boot time.
- **AWS spot (deferred):** on-demand only. Spot with `MaxPrice` = admitted price
  would keep the budget true but can interrupt a Task mid-run; needs Jack's call.
- **Prices (assumed):** RunPod CPU flavors publish no price; estimates
  $0.03/$0.04/$0.05 per vCPU-h (cpu3c/g/m), and the create's `costPerHr`
  replaces it on confirm. EC2 rates are us-east-1 on-demand; other regions
  differ and EC2 reports no price, so `lf ondemand list` may be off there.
  Modal GPU prices recorded from memory, unverified.
- **Fly:** one app per rental, its name the provider id; a dedicated IPv4 per
  app. A Machine that exits is destroyed (`auto_destroy`) but its app and
  IPv4 ($2/month) remain until lf's next refresh deletes the app. Fly sizes
  disk itself (`--disk-gb` ignored). `FLY_ORG` defaults to `personal`.
- **RunPod GPU pods:** `--vcpus`/`--memory-gb` are not sent with `--gpu`; the
  GPU type decides them.
- **DigitalOcean (assumed):** REST instead of `pydo`, like RunPod and Fly.
  Dedicated-CPU sizes only (c-, g-, m-; basic shared-CPU Droplets left out for
  builds), cheapest that holds the request; prices are monthly ÷ 672 h from
  memory, unverified. Each size has its own disk, so `--disk-gb` is ignored.
  Region from `DIGITALOCEAN_REGION`, default `nyc3`. Names map `_` to `-`.
  No account SSH key is registered: cloud-init admits `lf` and turns sshd
  key-only, but DigitalOcean may still email a root password at create.
  No guardian (Jack, open): a Droplet lf loses lives until the next reconcile.
