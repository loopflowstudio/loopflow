# Assumptions

- Agent ancestry matches `codex`/`claude` by executable name. A Claude Code install run through `node` (npm) isn't recognized by name; it still counts as an agent when it descends from `lf session serve`.
- A real build an agent starts (cargo under `claude`) now holds output only through open files, as the brief describes. Cargo keeps `target/*/.cargo-lock` and compiler outputs open while building, so live builds stay "held open".
