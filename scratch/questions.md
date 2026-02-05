# Open Questions

- GitHub polling (PR/CI status) from `scratch/01-lfd-mobile-prep.md` is not implemented yet. Do we want to port the existing Python `lfd/pr_poller.py` behavior into Rust, and which GitHub auth/token source should it use?
- TLS setup for remote HTTP/WSS (self-signed cert at `~/.lfd/server.{crt,key}`) is not implemented. Should lfd generate/manage certs, or rely on external TLS termination (e.g., Tailscale MagicDNS)?
- HTTP auth currently validates the loopflow.studio connection token (same as gRPC), not the raw JWT. Is the mobile client expected to pass the connection token instead of the JWT for lfd requests?
