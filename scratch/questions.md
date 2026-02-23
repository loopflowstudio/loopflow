# Open questions

- Codex app-server JSON-RPC payload shape was inferred (`turn/start` params, tool item fields, approval response format). Validate against real codex traces and adjust mapping if needed.
- Session startup currently returns `starting` immediately and transitions to `active` asynchronously. Confirm this is the intended API behavior.
