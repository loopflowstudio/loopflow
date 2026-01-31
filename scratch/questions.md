# Open Questions

- LfdId validates UUIDs, but step-run and fork-run IDs are currently composite strings (e.g., "{wave_id}:{step_index}"). I used `LfdId::from_raw(...)` for those internal IDs to avoid validation failures. Should we migrate step/fork IDs to UUIDs instead (and update engine + protocol), or relax validation for those types?
