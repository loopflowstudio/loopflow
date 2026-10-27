# LOO-624 open questions and assumptions

- **Images (assumed):** RunPod and Fly boot stock `ubuntu:24.04` and install
  TOOLS at boot instead of a prebuilt registry image, so no registry or image
  build is needed yet. Cost: minutes per boot (SSH_WAIT raised to 20 min). A
  pushed image is the follow-up once a live run shows the boot time.
- **Prices (assumed):** RunPod CPU flavors publish no price; estimates
  $0.03/$0.04/$0.05 per vCPU-h (cpu3c/g/m), and the create's `costPerHr`
  replaces it on confirm. EC2 rates are us-east-1 on-demand; other regions
  differ and EC2 reports no price, so `lf ondemand list` may be off there.
  Modal GPU prices recorded from memory, unverified.
- **Fly:** one app per rental, its name the provider id; a dedicated IPv4 per
  app. A Machine that exits is destroyed (`auto_destroy`) but its app and
  IPv4 ($2/month) remain until lf's next refresh deletes the app. Fly sizes
  disk itself (`--disk-gb` ignored). `FLY_ORG` defaults to `personal`.
- **Spot price (assumed):** a spot machine is admitted, capped and given its
  deadline at the on-demand price, the most it can be charged. RunPod's REST
  create takes `interruptible` without a bid; its reported `costPerHr` and the
  guardian then use the lower price. EC2 reports none, so `lf ondemand list`
  overstates an AWS spot machine's cost.
- **RunPod GPU floors (assumed):** RunPod checks `minVCPUPerGPU`/`minRAMPerGPU`
  at create; lf has no per-GPU host table, so an unmet floor fails the create
  (the rental is lost after 20 minutes) rather than at admission.
- **Region (assumed):** only AWS names one. Fly picks its own region and
  `FLY_ORG` is an organization, not a region; Fly prices are treated as uniform.