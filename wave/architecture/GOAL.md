---
primary_flow: build
---

Collapse the three binaries — `lf`, `lfd`, `lfq` — toward **one workhorse plus one
thin server**. `lf` already does the real behavior (run steps/flows, `lf goal`
runs a wave loop) and needs shell/binary/ssh access; that is the only access
model. Everything lfd and lfq do that is *exec-behavior* moves into `lf` (`lf d`
for store reads/writes + tmux/docker launch, `lf q` for queue/worker). `lfd`
shrinks to a **guarded subscription server** whose sole justification is push —
the live wave-status and terminal-output streams Concerto needs, which a transient
`lf` invocation cannot be. It execs `lf`; it does not reimplement behavior. This
is a reduce wave: each note should leave the system smaller and more true. Not a
rewrite — a collapse of concepts, net-negative code each pass.

**Metrics to improve**
- Binaries: 3 → 2 (`lf`, `lfd serve`). `lfq` deleted.
- lfd LOC: only the subscription hub + guarded external interface survives; the
  query API and executor move to `lf`. Target: net-negative on every landed item.
- Behavior reimplemented across binaries: 0 — `lfd serve` execs `lf`, never
  duplicates it.
- `lf`-launched sessions visible in live status: 0% → 100% (via the registry).

**Milestones**
- `lfdb` extracted: persistence is shared infra, not lfd-owned; lfd is one client.
- Session registry: every `lf` session self-registers; "active agents by
  worktree" is a real query.
- `lf d` / `lf q` absorb lfd's query+exec and lfq's queue; `lfq` binary deleted.
- Hard cut: `lfd serve` shrinks to the subscription hub + guarded interface,
  execing `lf` for launch; the old HTTP executor is gone.
