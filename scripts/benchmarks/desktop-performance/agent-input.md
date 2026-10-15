# Native agent input

```sh
# Set this only on the new owned demo app, before launching it.
export LOOPFLOW_AGENT_INPUT_TRACE=/tmp/agent-input-lf.ndjson

# Start recording before one sidebar click. Wait for xctrace to begin recording.
uv run python scripts/benchmarks/desktop-performance/agent_input.py record \
  --pid 123 --provider-pid 456 --metadata /tmp/trial.json \
  --lf-events /tmp/agent-input-lf.ndjson --output /tmp/codex-resume-before

# In another terminal, preserve a native capture at each observation.
# Mark the probe before typing one printable character. Leave it unsent.
uv run python scripts/benchmarks/desktop-performance/agent_input.py mark \
  /tmp/codex-resume-before input_visible --evidence /tmp/input-visible.png
uv run python scripts/benchmarks/desktop-performance/agent_input.py mark \
  /tmp/codex-resume-before probe_sent --evidence /tmp/before-probe.png
uv run python scripts/benchmarks/desktop-performance/agent_input.py mark \
  /tmp/codex-resume-before input_response --evidence /tmp/rendered-draft.png

uv run python scripts/benchmarks/desktop-performance/agent_input.py summarize /tmp/codex-resume-before
uv run python scripts/benchmarks/desktop-performance/agent_input.py compare \
  /tmp/codex-resume-before /tmp/codex-resume-after
```

Use disposable, owned conversations. Never type into retained drafts, submit the
probe, approve trust implicitly, restart a provider, or acquire another client's
conversation for a recording. The recorder launches no app, lf command or provider.
A cached trial requires a previously rendered native provider surface; a resume
trial has saved provider history without a running client; a fresh trial has none.
Rust's `native_resume_selected`/`fresh_launch_selected` milestones reveal its actual
choice. Include explicit provider PIDs for existing clients outside Desktop's
process ancestry. Process polling discovers new children, including lf and relay.

One recording covers one Task click and one provider/case. Repeat all six cases:
Codex and Claude × cached, resume, fresh. Use comparable cold/warm conditions,
Machine, data and provider version before/after. No historic read median supplies
a baseline. Recording a native draft response is still evidence for review,
not automatic proof that the pixels depict the intended provider input.

`trial.json` supplies exact identities and provenance, rather than discovering or
mutating Work:

```json
{
  "provider": "codex",
  "provider_version": "record exact version",
  "case": "resume",
  "conditions": "warm OS cache; idle host; no existing provider; owned fixture",
  "app_revision": "record exact built commit and build mode",
  "helper_revision": "record exact bundled lf commit and build mode",
  "task": "exact Task ID from the sidebar",
  "session": "exact owned primary Session ID",
  "machine": "exact selected Machine ID",
  "data": "exact data directory and fixture revision",
  "variant": "before"
}
```

Save raw evidence outside Git. The output directory is private and contains the
all-process Time Profiler trace, exported CPU stacks/TOC, process CPU-time snapshots,
Desktop signposts, copied lf wall events, observer markers, report, folded stacks
and `cpu.svg`. The SVG groups Running samples by process, with full stacks on
hover. Unexported stacks and missing symbols stay explicit; sample counts are not
CPU milliseconds. Inspect `recorder.log` and the raw trace to establish profiler
coverage: xctrace setup can miss early work, polling can miss short-lived children,
and an existing provider needs an explicit PID. CPU deltas cover only the first
to last observed process snapshot; preexisting CPU and missed startup are excluded. All-process tracing includes other
local processes; only selected ancestry/PIDs enter the flamegraph.

The endpoint is the capture of visible provider input responding to the unsent
probe, after an exact-Session key event and draw. Surface creation, first draw and
focus alone leave elapsed time null. Missing, repeated, superseded, out-of-range,
failed or interrupted evidence also remains unproved. Mark `failed` with its receipt;
mark `hook_review_begin` and `hook_review_end` with captures when applicable. Hook
review duration is reported alongside total time; it is never subtracted. An
unfinished review stays unmeasured.

Phase durations overlap and must not be summed. lf signpost spans include subprocess
waits; Rust wall events expose lock wait and launch selection/spawn. Local CPU stacks
and cumulative CPU snapshots complement those wall intervals. Network/I/O waits and
closed provider internals have no fabricated spans; inspect raw trace scheduling
and record any unresolved gap. No signpost or file is a readiness lifecycle, launch
condition, ownership fence or Session state.

The recorder adds all-process sampling, 100 ms process polling and optional small
lf file appends. Marker times include capture and observer delay. Run both variants
with identical observer settings; report those limits with the measured difference.
No numeric performance target is imposed. A missing display, trust review, profiling
permission or unavailable symbol/export facility leaves a named demo proof gap.
