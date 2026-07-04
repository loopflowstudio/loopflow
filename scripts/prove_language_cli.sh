#!/usr/bin/env bash
set -euo pipefail

lf_bin="${LF_BIN:-lf}"
probe_dir="$(mktemp -d "${TMPDIR:-/tmp}/loopflow-greenfield-cli.XXXXXX")"

cat >"${probe_dir}/GOAL.md" <<'GOAL'
# Goal Hello

Create a Rust CLI named `goal-hello`.

It accepts `--name <value>` and prints `hello, <value>`.

Done when `cargo run --quiet -- --name Loopflow` prints exactly
`hello, Loopflow`.

Do not author any `.lf/steps/*.md` files.
GOAL

echo "probe: ${probe_dir}"
(
  cd "${probe_dir}"
  "${lf_bin}" -b greenfield
)

steps_count=0
if [ -d "${probe_dir}/.lf/steps" ]; then
  steps_count="$(find "${probe_dir}/.lf/steps" -type f -name '*.md' | wc -l | tr -d ' ')"
fi

if [ "${steps_count}" != "0" ]; then
  echo "language failed: ${steps_count} local step(s) were authored in ${probe_dir}/.lf/steps" >&2
  exit 1
fi

output="$(
  cd "${probe_dir}"
  cargo run --quiet -- --name Loopflow
)"

echo "${output}"

if [ "${output}" != "hello, Loopflow" ]; then
  echo "language failed: expected 'hello, Loopflow'" >&2
  exit 1
fi

echo "language passed: CLI ran with 0 authored steps"
