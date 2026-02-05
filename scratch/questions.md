# Open Questions

- GitHub polling (PR/CI status) is not implemented in Rust yet. Port the existing Python `lfd/pr_poller.py` behavior, or rewrite? Which GitHub auth/token source should it use?
- TLS for remote HTTP/WSS: should lfd generate/manage self-signed certs at `~/.lfd/server.{crt,key}`, or rely on external TLS termination (e.g., Tailscale MagicDNS)?
- HTTP auth validates the loopflow.studio connection token, not raw JWT. Is the mobile client expected to obtain a connection token from loopflow.studio before calling lfd?
- Remote HTTP access is blocked when loopflow.studio registration isn't enabled or hasn't completed. Is there a desired fallback for remote access without registration?
