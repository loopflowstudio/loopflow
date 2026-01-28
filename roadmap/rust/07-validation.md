# Rust Roadmap: Validation + Experiments (Stage 7)

Prove the Rust engine meets reliability, performance, and parity goals.

## Goal
Run experiments that justify the rewrite and define go/no-go gates.

## Scope
- Shadow-mode parity testing
- Synthetic load tests
- Failure injection
- Cluster portability trials

## Experiments
1. **Rust daemon prototype**: minimal socket server + read‑only DB.
2. **Parity suite**: compare Rust engine output vs Python for representative flows.
3. **Load test**: simulate 1000+ loops with fake agents.
4. **Failure injection**: crash runs, DB locks, network drops.
5. **Remote control**: `lf` on laptop controlling a remote daemon.
6. **Cross‑platform build**: macOS + Linux binaries, size + startup time.
7. **Token counting check**: compare Rust token counts vs Python.
8. **Full step run**: Rust context assembly + agent launch parity.

## Metrics
- Scheduling jitter
- CPU/RAM per loop
- Error rate per 100 runs
- Recovery time after failures

## Success criteria
- Parity ≥ 95% on golden flow set.
- Jitter reduction ≥ 30% vs Python.
- No data loss under crash tests.

## Open questions
- What is the canonical "golden flow" set?
- What level of parity is acceptable if behavior changes are improvements?
