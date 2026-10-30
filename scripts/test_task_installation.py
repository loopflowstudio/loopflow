"""Run Task CLI proofs with disposable OS installation authority."""

import argparse
import io
import subprocess
import tarfile
import time
from pathlib import Path

CANONICAL_IDS = "store::migrations::canonical_ids_tests"

PROOFS = {
    "running_provider_uses_updated_install_for_commands_hooks_and_graph": (
        "os_process::installation_tests"
    ),
    "graph_live_across_switch_runs_next_step_on_selected_install": (
        "os_process::installation_tests"
    ),
    "task_adopts_linear_checkout_and_preserves_graph_history": "task_adoption_tests",
    "declared_agent_can_start_another_tasks_graph": "session_lifecycle_tests",
    "task_graph_read_keeps_captured_topology_and_counts_both_returns": "graph_tests",
    "installation_uses_candidate_authority_from_any_checkout": "global_commands",
    "installation_reaches_candidate_verdict_with_an_unreadable_task_registry": "global_commands",
    "install_preflight_records_no_process": "process_ownership_tests",
    # Structured-ID upgrade on a case-sensitive filesystem with live owners.
    "canonical_ids_live_receipt_defers_then_normal_settlement_allows_upgrade": CANONICAL_IDS,
    "canonical_ids_changed_claim_waits_for_canonical_live_caller": CANONICAL_IDS,
    "canonical_ids_canonical_live_claim_does_not_defer_upgrade": CANONICAL_IDS,
    "canonical_ids_retained_receipt_and_trace_locator_survive_identity_change": CANONICAL_IDS,
    "canonical_ids_commands_read_the_upgraded_work_and_native_history": CANONICAL_IDS,
}


LIB_PROOFS = {
    "task_adoption_tests",
    "graph_tests",
    "os_process::installation_tests",
    CANONICAL_IDS,
}


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


def _wait_for_docker() -> None:
    """Wait out a busy daemon; fail before creating resources when it stays stuck."""
    # Hosted runners can stall the daemon briefly, e.g. right after a large cache restore.
    deadline = time.monotonic() + 120
    while True:
        try:
            subprocess.run(
                ["docker", "info", "--format", "{{.ServerVersion}}"], check=True, timeout=10
            )
            return
        except (subprocess.TimeoutExpired, subprocess.CalledProcessError):
            if time.monotonic() >= deadline:
                raise SystemExit(
                    "Infrastructure failure: Docker did not respond within 120 seconds; "
                    "no proof container was created."
                ) from None
        print("Docker is not answering yet; retrying in 5 seconds.", flush=True)
        time.sleep(5)


def _migration_order(repo: Path) -> str:
    """Every slug migration in this tree: origin/main's merge order, then the rest by name."""
    directory = "rust/loopflow/src/store/migrations"
    log = subprocess.run(
        [
            "git",
            "log",
            "origin/main",
            "--first-parent",
            "--diff-merges=first-parent",
            "--no-renames",
            "--diff-filter=A",
            "--reverse",
            "--name-only",
            "--format=",
            "--",
            directory,
        ],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    present = {path.stem for path in (repo / directory).glob("[a-z]*.sql")}
    merged = [Path(line).stem for line in log.stdout.splitlines() if Path(line).stem in present]
    order = list(dict.fromkeys(merged)) + sorted(present - set(merged))
    return "".join(f"{slug}\n" for slug in order)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", default="docker.io/library/rust:trixie")
    parser.add_argument("--test", nargs="+", choices=PROOFS, help="run selected named proofs")
    parser.add_argument(
        "--cache-dir",
        type=Path,
        help="persist Cargo target and registry on a native Linux filesystem",
    )
    parser.add_argument(
        "--repository-upgrade",
        action="store_true",
        help="prove populated released repository upgrade",
    )
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    _wait_for_docker()
    _prepare_image(args.image)
    mounts = [
        "type=volume,source=loopflow-task-proof-target,target=/source/target",
        "type=volume,source=loopflow-task-proof-registry,target=/usr/local/cargo/registry",
    ]
    if args.cache_dir:
        mounts = []
        for name, destination in (
            ("target", "/source/target"),
            ("registry", "/usr/local/cargo/registry"),
        ):
            source = args.cache_dir.resolve() / name
            source.mkdir(parents=True, exist_ok=True)
            mounts.append(f"type=bind,source={source},target={destination}")
    container = subprocess.check_output(
        [
            "docker",
            "create",
            "--mount",
            mounts[0],
            "--mount",
            mounts[1],
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
                # The snapshot has no Git history; it builds as the main commit it
                # would merge into, so its migrations count as merged.
                order = _migration_order(repo).encode()
                member = tarfile.TarInfo("source/migration-order.txt")
                member.size = len(order)
                archive.addfile(member, io.BytesIO(order))
            assert copy.stdin is not None
            copy.stdin.close()
            if copy.wait() != 0:
                raise RuntimeError("copy disposable source snapshot")
        selected = {name: PROOFS[name] for name in args.test} if args.test else PROOFS
        targets = " ".join(
            sorted({f"--test {target}" for target in selected.values() if target not in LIB_PROOFS})
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
    LOOPFLOW_MIGRATION_ORDER=/source/migration-order.txt \
    CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 \
    flock /source/target/.installation-proof.lock sh -ec 'cd /source
        rustc --version
        PREPARE
        '
"""
        prepare = "nice -n 10 cargo build -p loopflow --bin lf"
        if not args.repository_upgrade:
            # Integration targets build the CLI too. A library-only selection
            # still needs an explicit build before entering network isolation.
            build_tests = f"nice -n 10 cargo test -p loopflow --lib {targets} --no-run"
            prepare = build_tests if targets else f"{build_tests}\n{prepare}"
        command = command.replace("PREPARE", prepare)
        if args.repository_upgrade:
            command = command.replace(
                "LOOPFLOW_BUILD_PROVENANCE=development",
                "LOOPFLOW_BUILD_PROVENANCE=release",
            )
        subprocess.run(
            ["docker", "exec", container, "sh", "-ec", command], check=True, timeout=1800
        )
        if args.repository_upgrade:
            subprocess.run(
                [
                    "docker",
                    "exec",
                    container,
                    "python3",
                    "/source/tests/e2e/repository_upgrade.py",
                    "--fetch-prior",
                ],
                check=True,
                timeout=300,
            )
        # Dependency downloads and compilation are complete. Docker keeps loopback
        # while disconnecting the only external interface, including descendants.
        subprocess.run(["docker", "network", "disconnect", "bridge", container], check=True)
        isolated = r"""
runuser -u lf-task-proof -- env HOME=/home/lf-task-proof GIT_ALLOW_PROTOCOL=file \
    LOOPFLOW_TEST_LF=/source/target/debug/lf \
    LOOPFLOW_BUILD_PROVENANCE=development CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 \
    LOOPFLOW_MIGRATION_ORDER=/source/migration-order.txt \
    CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 \
    flock /source/target/.installation-proof.lock sh -ec 'cd /source
        PYTHONPATH=scripts python3 -c "from test_network import _probe; _probe()"
        timeout 180 cargo test -p loopflow --lib \
            migration_preserves_planning_identity_and_removes_snapshot_storage
        timeout 180 cargo test -p loopflow --lib \
            optional_task_pr_preserves_placement_and_freezes_prior_delivery
        CHECKS'
""".replace("CHECKS", checks)
        if args.repository_upgrade:
            isolated = (
                "cd /source && PYTHONPATH=scripts python3 -c "
                "'from test_network import _probe; _probe()' && "
                "python3 tests/e2e/repository_upgrade.py /source/target/debug/lf"
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
