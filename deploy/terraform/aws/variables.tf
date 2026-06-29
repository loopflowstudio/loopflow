variable "region" {
  description = "AWS region for the loopflow host."
  type        = string
  default     = "us-west-2"
}

variable "name" {
  description = "Name prefix for loopflow server resources."
  type        = string
  default     = "loopflow-server"
}

variable "instance_type" {
  description = "EC2 instance type. Use enough memory for Rust package builds and Docker agents."
  type        = string
  default     = "t4g.medium"
}

variable "ssh_key_name" {
  description = "Existing EC2 key pair for emergency SSH access."
  type        = string
}

variable "allowed_ssh_cidrs" {
  description = "CIDRs allowed to SSH to the host. Prefer a Tailscale subnet or your current IP."
  type        = list(string)
  default     = []
}

variable "allowed_https_cidrs" {
  description = "CIDRs allowed to reach Caddy on 80/443. Use [] for Tailscale-only/private access."
  type        = list(string)
  default     = ["0.0.0.0/0"]
}

variable "repo_url" {
  description = "Loopflow git repository URL cloned onto the host."
  type        = string
  default     = "https://github.com/loopflowstudio/loopflow.git"
}

variable "repo_dir" {
  description = "Path where the repo is cloned on the host."
  type        = string
  default     = "/opt/loopflow"
}
