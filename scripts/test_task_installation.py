"""Run managed Task CLI proofs with disposable OS installation authority."""

import argparse
import subprocess
import tarfile
from pathlib import Path

PROOFS = {
    "installation_switch_preserves_task_review_without_store_overrides": "task_initialization_tests",
    "flow_step_executable_falls_back_without_losing_its_store": "flow_tests",
    "declared_agent_can_start_another_tasks_flow": "session_cutover_tests",
    "task_resume_revokes_auto_merge_before_returning_to_human_review": "pr_tests",
    "direct_open_preserves_another_installations_development_store": "task_initialization_tests",
    "incompatible_branch_data_recommends_only_a_verified_retained_pair": (
        "task_initialization_tests"
    ),
    "task_review_completion_consumes_only_installed_readiness": "task_initialization_tests",
    "task_operation_starts_with_durable_history_after_claim_only_failure": "flow_tests",
    "task_flow_read_pins_topology_counts_both_returns_and_rejects_a_bad_restart": "flow_tests",
}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", default="rust:1.89-bookworm")
    parser.add_argument("--test", choices=PROOFS, help="run one named proof")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    # Fail before creating resources when the shared container service is stuck.
    try:
        subprocess.run(["docker", "info", "--format", "{{.ServerVersion}}"], check=True, timeout=10)
    except subprocess.TimeoutExpired:
        raise SystemExit(
            "Docker did not respond within 10 seconds; no proof container was created."
        ) from None
    # A cold runner downloads the image before it can create a container.
    # Keep that network transfer outside the short local startup deadline.
    subprocess.run(["docker", "pull", args.image], check=True, timeout=300)
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
        # Copy source, never mount a host Home, installation or Cargo credentials.
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
        selected = {args.test: PROOFS[args.test]} if args.test else PROOFS
        targets = " ".join(f"--test {target}" for target in sorted(set(selected.values())))
        checks = "\n".join(
            f"timeout 180 cargo test -p loopflow --test {target} {name} "
            "-- --exact --ignored --nocapture"
            for name, target in selected.items()
        )
        command = r"""
set -eu
useradd --create-home lf-task-proof
chown -R lf-task-proof:lf-task-proof /source
chown -R lf-task-proof:lf-task-proof /usr/local/cargo/registry
runuser -u lf-task-proof -- env HOME=/home/lf-task-proof \
    LOOPFLOW_BUILD_PROVENANCE=development CARGO_INCREMENTAL=0 \
    CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_BUILD_JOBS=4 \
    flock /source/target/.installation-proof.lock sh -ec 'cd /source
        nice -n 10 cargo test -p loopflow TARGETS --no-run
        CHECKS'
"""
        command = command.replace("TARGETS", targets).replace("CHECKS", checks)
        subprocess.run(
            ["docker", "exec", container, "sh", "-ec", command], check=True, timeout=1800
        )
    finally:
        subprocess.run(["docker", "rm", "--force", container], check=True)


if __name__ == "__main__":
    main()
