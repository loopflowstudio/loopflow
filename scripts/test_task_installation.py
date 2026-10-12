"""Run Task CLI proofs with disposable OS installation authority."""

import argparse
import subprocess
import tarfile
import time
from pathlib import Path

PROOFS = {
    "default_and_nested_commands_use_the_installed_cli_and_main_home": "one_machine_tests",
    "task_adopts_linear_checkout_and_preserves_flow_history": "task_adoption_tests",
    "declared_agent_can_start_another_tasks_flow": "session_lifecycle_tests",
    "task_flow_read_keeps_captured_topology_and_counts_both_returns": "flow_tests",
    "installation_uses_candidate_authority_from_any_checkout": "global_commands",
    "installation_reaches_candidate_verdict_with_an_unreadable_task_registry": "global_commands",
    "early_observation_records_preflight": "process_ownership_tests",
}


LIB_PROOFS = {"task_adoption_tests", "flow_tests"}


def _prepare_image(image: str) -> None:
    failure = (
        f"Infrastructure failure acquiring proof image {image}; installation tests did not run."
    )
    try:
        cached = subprocess.run(
            ["docker", "image", "inspect", image],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=10,
        )
        if cached.returncode == 0:
            print(f"Using local proof image {image}", flush=True)
            return

        # Download and backoff share the cold-runner deadline, separate from startup.
        deadline = time.monotonic() + 300
        for delay in (15, 30, 60, None):
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise SystemExit(f"{failure} Image download exceeded 300 seconds.")
            result = subprocess.run(
                ["docker", "pull", image],
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                timeout=remaining,
            )
            print(result.stdout, end="", flush=True)
            if result.returncode == 0:
                return
            throttled = any(
                marker in result.stdout.lower()
                for marker in ("toomanyrequests", "too many requests", "rate exceeded")
            )
            if not throttled:
                raise SystemExit(failure)
            if delay is None or deadline - time.monotonic() <= delay:
                raise SystemExit(
                    f"{failure} Registry rate limit persisted through bounded retries."
                )
            print(f"Registry rate limit for {image}; retrying in {delay} seconds.", flush=True)
            time.sleep(delay)
    except subprocess.TimeoutExpired:
        raise SystemExit(f"{failure} Docker image acquisition timed out.") from None


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", default="public.ecr.aws/docker/library/rust:trixie")
    parser.add_argument("--test", nargs="+", choices=PROOFS, help="run selected named proofs")
    parser.add_argument(
        "--native-titles", action="store_true", help="prove published title hook installation"
    )
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    # Fail before creating resources when the shared container service is stuck.
    try:
        subprocess.run(["docker", "info", "--format", "{{.ServerVersion}}"], check=True, timeout=10)
    except subprocess.TimeoutExpired:
        raise SystemExit(
            "Docker did not respond within 10 seconds; no proof container was created."
        ) from None
    _prepare_image(args.image)
    container = subprocess.check_output(
        [
            "docker",
            "create",
            "--mount",
            "type=volume,source=loopflow-task-proof-target,target=/source/target",
            "--mount",
            "type=volume,source=loopflow-task-proof-registry,target=/usr/local/cargo/registry",
            args.image,
            "sleep",
            "infinity",
        ],
        text=True,
        timeout=30,
    ).strip()
    try:
        subprocess.run(["docker", "start", container], check=True)
        paths = subprocess.check_output(
            ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
            cwd=repo,
        )
        # Copy source, never mount a host Machine, installation or Cargo credentials.
        with subprocess.Popen(
            ["docker", "cp", "-", f"{container}:/"], stdin=subprocess.PIPE
        ) as copy:
            with tarfile.open(fileobj=copy.stdin, mode="w|") as archive:
                for raw in sorted(set(paths.split(b"\0")) - {b""}):
                    path = Path(raw.decode())
                    if (repo / path).is_file():
                        archive.add(repo / path, arcname=str(Path("source") / path))
            assert copy.stdin is not None
            copy.stdin.close()
            if copy.wait() != 0:
                raise RuntimeError("copy disposable source snapshot")
        selected = {name: PROOFS[name] for name in args.test} if args.test else PROOFS
        targets = " ".join(
            sorted(
                {
                    "--lib" if target in LIB_PROOFS else f"--test {target}"
                    for target in selected.values()
                }
            )
        )
        checks = "\n".join(
            f"timeout 180 cargo test -p loopflow "
            f"{'--lib' if target in LIB_PROOFS else f'--test {target}'} "
            f"{f'{target}::' if target in LIB_PROOFS else ''}{name} "
            "-- --exact --include-ignored --nocapture"
            for name, target in selected.items()
        )
        command = r"""
set -eu
useradd --create-home lf-task-proof
chown -R lf-task-proof:lf-task-proof /source
chown -R lf-task-proof:lf-task-proof /usr/local/cargo/registry
runuser -u lf-task-proof -- env HOME=/home/lf-task-proof \
    LOOPFLOW_BUILD_PROVENANCE=development CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 \
    CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 \
    flock /source/target/.installation-proof.lock sh -ec 'cd /source
        rustc --version
        version=$(sed -n "s/^version = \"\([^\"]*\)\"/\1/p" Cargo.toml | head -1)
        python3 scripts/canonicalize_migrations.py "$version" --materialize-for-tests
        nice -n 10 cargo test -p loopflow --lib --no-run
        nice -n 10 cargo test -p loopflow TARGETS --no-run
        '
"""
        command = command.replace("TARGETS", targets)
        if args.native_titles:
            command = command.replace(
                "LOOPFLOW_BUILD_PROVENANCE=development",
                "LOOPFLOW_BUILD_PROVENANCE=release LOOPFLOW_MIGRATION_AUTHORITY=published",
            ).replace(
                "nice -n 10 cargo test -p loopflow --lib --no-run\n"
                f"        nice -n 10 cargo test -p loopflow {targets} --no-run",
                "nice -n 10 cargo build -p loopflow --bin lf",
            )
        subprocess.run(
            ["docker", "exec", container, "sh", "-ec", command], check=True, timeout=1800
        )
        # Dependency downloads and compilation are complete. Docker keeps loopback
        # while disconnecting the only external interface, including descendants.
        subprocess.run(["docker", "network", "disconnect", "bridge", container], check=True)
        isolated = r"""
runuser -u lf-task-proof -- env HOME=/home/lf-task-proof GIT_ALLOW_PROTOCOL=file \
    LOOPFLOW_BUILD_PROVENANCE=development CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 \
    CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 \
    flock /source/target/.installation-proof.lock sh -ec 'cd /source
        PYTHONPATH=scripts python3 -c "from test_network import _probe; _probe()"
        timeout 180 cargo test -p loopflow --lib \
            migration_preserves_planning_identity_and_removes_snapshot_storage
        timeout 180 cargo test -p loopflow --lib \
            optional_task_pr_preserves_placement_and_freezes_prior_delivery
        CHECKS'
""".replace("CHECKS", checks)
        if args.native_titles:
            isolated = (
                "cd /source && PYTHONPATH=scripts python3 -c "
                "'from test_network import _probe; _probe()' && "
                "python3 tests/e2e/native_title_install.py /source/target/debug/lf"
            )
        subprocess.run(
            ["docker", "exec", container, "sh", "-ec", isolated], check=True, timeout=1800
        )
    finally:
        try:
            subprocess.run(["docker", "rm", "--force", container], check=True, timeout=30)
        except subprocess.TimeoutExpired:
            raise RuntimeError(
                f"Docker cleanup timed out; proof container {container} still needs removal."
            ) from None


if __name__ == "__main__":
    main()
