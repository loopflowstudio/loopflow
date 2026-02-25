# EC2 Dogfood Lane — Close-the-Gap Checklist

_Last updated: February 25, 2026_

This checklist closes the gap between the current `lfd-dev` state (`44.227.252.151`) and the Step 1 target in `wave/remote`.

## Current state snapshot (verified February 25, 2026)

- `lfd-dev` host is reachable and serving `https://44.227.252.151/health`.
- TLS is currently Caddy **internal CA** (not public ACME cert).
- Remote host `/home/ubuntu/loopflow` is on `49914dd` and **15 commits behind** `origin/main` (`32cfda7`).
- Remote `deploy/Caddyfile` is manually edited (hardcoded IP), creating drift.
- `../studio` `main` includes `scripts/lfq_e2e.py` + `terraform/dev/deploy.py redeploy`.
- `loopflow.remote` still lacks the planned Step 1 deliverables (param Caddyfile, prod compose env+80, remote smoke script, deploy README).

---

## Phase 1 — Stabilize `lfd-dev` deployment lane (`../studio`)

- [ ] **P1.1 Remove hidden host drift before redeploy**
  - Repo: `../studio`
  - Action: SSH to `lfd-dev`, capture and commit any intentional host-only config deltas (or delete them).
  - Done when: `cd /home/ubuntu/loopflow && git status --short` is clean.

- [ ] **P1.2 Redeploy remote host from known branch tip**
  - Repo: `../studio`
  - Command: `python3 terraform/dev/deploy.py redeploy main`
  - Done when: remote `git rev-parse HEAD` equals `origin/main`, compose services healthy.

- [ ] **P1.3 Keep stage verification runnable from studio**
  - Repo: `../studio`
  - Command: `python3 scripts/lfq_e2e.py all --stage4-url https://44.227.252.151 --stage4-token '<token>' --stage4-ca-cert <cert>`
  - Done when: all 4 stages pass on first run.

---

## Phase 2 — Ship Step 1 code in `loopflow.remote`

- [ ] **P2.1 Parameterize Caddy for dev/prod domain modes**
  - Repo: `loopflow.remote`
  - Files: `deploy/Caddyfile`
  - Required changes:
    - Use `{$LF_DOMAIN:localhost}` site label
    - Use `tls {$LF_TLS_MODE:internal}`
    - Add websocket matcher + dedicated reverse proxy
    - Add `flush_interval -1` for SSE
  - Done when: same file works for localhost/internal and domain/ACME.

- [ ] **P2.2 Expose ACME path + pass Caddy env in prod compose**
  - Repo: `loopflow.remote`
  - File: `deploy/docker-compose.prod.yml`
  - Required changes:
    - Add `LF_DOMAIN` + `LF_TLS_MODE` env to `caddy`
    - Expose both `443:443` and `80:80`
  - Done when: compose can run internal TLS and ACME modes without file edits.

- [ ] **P2.3 Add one-command remote smoke script**
  - Repo: `loopflow.remote`
  - File: `scripts/test_remote_smoke.py`
  - Required scenarios:
    1. `/health`
    2. wave CRUD
    3. auth rejection
    4. SSE session events
    5. WS `/ws` connected snapshot
    6. wave run + logs stream
    7. WS reconnect snapshot
  - Done when: script exits 0 only when all scenarios pass.

- [ ] **P2.4 Add deploy runbook for EC2 lane**
  - Repo: `loopflow.remote`
  - File: `deploy/README.md`
  - Include: prerequisites, quick start, env config, verification, credential mounts, troubleshooting, manual Concerto checks.
  - Done when: clean-room reprovision is possible without tribal knowledge.

- [ ] **P2.5 Add required Python dev dependency**
  - Repo: `loopflow.remote`
  - File: `pyproject.toml`
  - Required change: add `websockets` to dev dependency group if not already present.
  - Done when: smoke script WS path runs from `uv run`.

---

## Phase 3 — Converge `lfd-dev` onto Step 1 target behavior

- [ ] **P3.1 Deploy updated `loopflow.remote` to `lfd-dev`**
  - Repo: `../studio` + remote host
  - Command: `python3 terraform/dev/deploy.py redeploy <branch-with-step1-changes>`
  - Done when: host is running the new Caddy/compose configuration with no manual edits.

- [ ] **P3.2 Verify internal TLS mode on raw IP lane (current)**
  - Command: run remote smoke against `https://44.227.252.151` with CA cert bundle.
  - Done when: all 7 smoke scenarios pass over TLS proxy.

- [ ] **P3.3 Verify public-domain ACME lane (target)**
  - Prereq: DNS A record points domain to `44.227.252.151`, SG permits 80/443.
  - Command: run same remote smoke against `https://<domain>` with trusted cert.
  - Done when: all 7 scenarios pass without custom CA cert.

---

## Exit criteria (Step 1 complete)

- [ ] `loopflow.remote` has all four deliverables merged:
  - parameterized Caddyfile
  - prod compose domain+ACME wiring
  - remote smoke script
  - deploy README
- [ ] `lfd-dev` is reproducible from docs + scripts only (no hand edits on host).
- [ ] SSE and WS are validated through Caddy TLS by passing smoke scenarios 4/5/7.
- [ ] Team can run remote dogfood loop from laptop with one smoke command.

---

## Open questions to resolve during execution

- [ ] When will `lfd.loopflow.dev` (or equivalent) DNS be created and pointed at `44.227.252.151`?
- [ ] Should `terraform/dev/deploy.py redeploy` also enforce Caddy mode (`internal` vs ACME) to prevent config drift?
