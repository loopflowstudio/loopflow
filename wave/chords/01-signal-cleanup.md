# 01: Signal Cleanup

**Finish line:** `Signal` enum has only reactive variants (Watch, Listen, Cron, CiFailure). Waves have a `mode` field (Loop/Once). Loop ticker queries `wave.mode`, not Loop stimuli. All tests pass.

## Context

Default stimuli and the integrate flow shipped in the previous branch. The README already describes the target signal model — this sprint implements it.

## What changes

Remove `Once = 1` and `Loop = 2` from `Signal`. Keep discriminants stable (3, 4, 5, 6) for DB compatibility. `from_i32` returns `Unspecified` for legacy 1/2 rows.

Add `WaveMode` enum (Loop, Once) and `mode: WaveMode` to `Wave` struct. Default is `Loop`.

### DB migration (022_wave_mode.sql)

```sql
ALTER TABLE waves ADD COLUMN mode TEXT NOT NULL DEFAULT 'loop';
DELETE FROM stimuli WHERE signal IN (1, 2);
```

### Store layer

- `map_wave_row` reads `mode` column in both sqlite.rs and postgres.rs
- `InsertWave`/`UpdateWave` in catalog.rs include `mode`
- New query: `list_loopable_waves()` — waves where `mode = 'loop' AND status != 'paused'`
- Add to store trait + SharedStore

### Loop ticker

Replace `list_stimuli_by_signal(Signal::Loop)` with `list_loopable_waves()`. The ticker no longer needs a stimulus ID — make `ActivationEnvelope.stimulus_id` optional. Check all consumers.

### HTTP routes (waves.rs)

- Delete `ensure_manual_stimulus` — manual runs dispatch activations directly
- Remove `"loop"` and `"once"` from `parse_stimulus`
- Simplify `is_auto_stimulus` to `!matches!(signal, Signal::Unspecified)` or delete it
- `AddStimulusRequest` naturally rejects loop/once since they're gone from the enum

### Executor

`is_recurring` becomes `wave.mode == WaveMode::Loop || stimuli.any(Watch|Cron)`.

### WaveConfig + DTO

- Add `mode: Option<String>` to `WaveConfig`
- Add `mode` to wave DTO in dto.rs
- Remove `Loop`/`Once` arms from `signal_str`

### Python

- Update `conftest.py` test fixture from `"kind": "loop"` to a valid signal

## Uncertainty

- **ActivationEnvelope.stimulus_id** — making it `Option<LfdId>` is cleaner than a sentinel, but need to verify how it's used in activation logging and dedup. If dedup keys on stimulus_id, optional changes the behavior.
- **ensure_manual_stimulus** — deleting is cleanest, but check if anything reads the Once stimulus row after creation (activation logs, UI display).
