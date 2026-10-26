"""Time a loopflow development loop on a sandbox provider, reached over SSH.

    uv run --with modal scratch/sandboxes/trial.py modal
    doppler run -- uv run --with daytona scratch/sandboxes/trial.py daytona

Each provider only creates, addresses, snapshots and ends a sandbox; every
measured step runs over plain SSH, as lf's Machine transport would.
"""

import json
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
PUBLIC_KEY = (Path.home() / ".ssh" / "id_ed25519.pub").read_text().strip()
VCPUS = 8
MEMORY_GIB = 16
# Backstop: the provider ends the sandbox even if this script dies.
LIFETIME_SECONDS = 90 * 60

SSHD = (
    "mkdir -p /root/.ssh /run/sshd && chmod 700 /root/.ssh"
    " && printf '%s\\n' \"$AUTHORIZED_KEY\" > /root/.ssh/authorized_keys"
    " && chmod 600 /root/.ssh/authorized_keys"
    " && exec /usr/sbin/sshd -D -e -o PasswordAuthentication=no"
)
BUILD = "cd ~/src/loopflow && export PATH=$HOME/.cargo/bin:/usr/local/cargo/bin:$PATH && cargo build -p loopflow --bin lf 2>&1 | tail -3"
TOOLS = (
    "apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq"
    " openssh-server git curl ca-certificates build-essential pkg-config cmake clang"
)
RUSTUP = "curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal"


@dataclass
class Route:
    host: str
    port: int
    user: str = "root"

    def ssh(self, command: str, stdin=None, check: bool = True) -> str:
        result = subprocess.run(
            [
                "ssh", "-p", str(self.port),
                "-o", "StrictHostKeyChecking=no", "-o", "UserKnownHostsFile=/dev/null",
                "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "-o", "LogLevel=ERROR",
                f"{self.user}@{self.host}", command,
            ],
            stdin=stdin, capture_output=True, text=stdin is None, check=False,
        )
        out = result.stdout if isinstance(result.stdout, str) else result.stdout.decode()
        if check and result.returncode != 0:
            err = result.stderr if isinstance(result.stderr, str) else result.stderr.decode()
            raise RuntimeError(f"ssh {command!r} exited {result.returncode}: {err.strip()}")
        return out

    def wait(self, limit: float = 300) -> None:
        start = time.monotonic()
        while True:
            try:
                self.ssh("true")
                return
            except RuntimeError:
                if time.monotonic() - start > limit:
                    raise
                time.sleep(2)


class Modal:
    """Modal Sandboxes; `cpu` counts physical cores, two vCPUs each."""

    name = "modal"
    # https://modal.com/pricing, sandbox (non-preemptible) rates
    usd_per_core_second = 0.00003942
    usd_per_gib_second = 0.00000667

    def __init__(self) -> None:
        import modal

        self.modal = modal
        self.app = modal.App.lookup("lf-sandbox-trial", create_if_missing=True)
        self.image = (
            modal.Image.debian_slim(python_version="3.12")
            .apt_install(
                "openssh-server", "git", "curl", "ca-certificates", "build-essential",
                "pkg-config", "cmake", "clang",
            )
            .run_commands(
                "curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal",
            )
        )

    def create(self, image=None):
        return self.modal.Sandbox.create(
            "sh", "-c", SSHD,
            app=self.app,
            image=image or self.image,
            cpu=VCPUS / 2,
            memory=MEMORY_GIB * 1024,
            timeout=LIFETIME_SECONDS,
            unencrypted_ports=[22],
            secrets=[self.modal.Secret.from_dict({"AUTHORIZED_KEY": PUBLIC_KEY})],
        )

    def route(self, sandbox) -> Route:
        host, port = sandbox.tunnels()[22].tcp_socket
        return Route(host, port)

    def snapshot(self, sandbox):
        return sandbox.snapshot_filesystem()

    def end(self, sandbox) -> None:
        sandbox.terminate()

    def usd(self, seconds: float) -> float:
        return seconds * (
            VCPUS / 2 * self.usd_per_core_second + MEMORY_GIB * self.usd_per_gib_second
        )


class Daytona:
    """Daytona container sandboxes, reached through its SSH gateway with an
    expiring token. Container sandboxes cannot be snapshotted, so the second
    half resumes the stopped sandbox instead of creating from a snapshot."""

    name = "daytona"
    # https://www.daytona.io/pricing; org default maximum is 4 vCPU / 8 GiB / 10 GiB
    vcpus = 4
    memory_gib = 8
    usd_per_vcpu_second = 0.0504 / 3600
    usd_per_gib_second = 0.0162 / 3600

    def __init__(self) -> None:
        import daytona

        self.sdk = daytona
        self.client = daytona.Daytona(daytona.DaytonaConfig(target="us"))
        self.image = (
            daytona.Image.debian_slim("3.12")
            # The sandbox logs in as `daytona`, not the root that builds the image.
            .env({"RUSTUP_HOME": "/usr/local/rustup", "CARGO_HOME": "/usr/local/cargo"})
            .run_commands(TOOLS, RUSTUP, "chmod -R a+rwX /usr/local/rustup /usr/local/cargo")
        )

    def create(self, image=None):
        if image is not None:
            image.start(timeout=300)
            return image
        return self.client.create(
            self.sdk.CreateSandboxFromImageParams(
                image=self.image,
                resources=self.sdk.Resources(
                    cpu=self.vcpus, memory=self.memory_gib, disk=10
                ),
                target="us",
                auto_stop_interval=0,
                ttl_minutes=LIFETIME_SECONDS // 60,
            ),
            timeout=1200,
            on_snapshot_create_logs=lambda chunk: None,
        )

    def route(self, sandbox) -> Route:
        access = sandbox.create_ssh_access(expires_in_minutes=LIFETIME_SECONDS // 60)
        return Route("ssh.app.daytona.io", 22, access.token)

    def snapshot(self, sandbox):
        sandbox.stop(timeout=300)
        return sandbox

    def end(self, sandbox) -> None:
        self.client.delete(sandbox)

    def usd(self, seconds: float) -> float:
        return seconds * (
            self.vcpus * self.usd_per_vcpu_second + self.memory_gib * self.usd_per_gib_second
        )


@dataclass
class Trial:
    provider: object
    steps: dict = field(default_factory=dict)
    billed_seconds: float = 0.0
    notes: dict = field(default_factory=dict)

    def step(self, name: str, action):
        start = time.monotonic()
        result = action()
        self.steps[name] = round(time.monotonic() - start, 1)
        print(f"{name}: {self.steps[name]}s", flush=True)
        return result

    def run(self) -> None:
        live = []
        try:
            first_start = time.monotonic()
            first = self.step("create", self.provider.create)
            live.append(first)
            route = self.step("route", lambda: self.provider.route(first))
            self.step("ssh ready", route.wait)
            archive = subprocess.Popen(
                ["git", "-C", str(REPO), "archive", "--format=tar", "HEAD"],
                stdout=subprocess.PIPE,
            )
            self.step(
                "upload source",
                lambda: route.ssh("mkdir -p ~/src/loopflow && tar -x -C ~/src/loopflow",
                                  stdin=archive.stdout),
            )
            self.notes["cold build"] = self.step("cold build", lambda: route.ssh(BUILD)).strip()
            self.notes["target size"] = route.ssh("du -sh ~/src/loopflow/target").split()[0]
            self.notes["nproc"] = route.ssh("nproc").strip()
            snapshot = self.step("snapshot", lambda: self.provider.snapshot(first))
            if snapshot is not first:
                self.step("end first", lambda: self.provider.end(first))
                live.remove(first)
            self.billed_seconds += time.monotonic() - first_start

            second_start = time.monotonic()
            second = self.step("create from snapshot",
                               lambda: self.provider.create(snapshot))
            if second not in live:
                live.append(second)
            route = self.provider.route(second)
            self.step("ssh ready from snapshot", route.wait)
            route.ssh("touch ~/src/loopflow/rust/loopflow/src/rental/mod.rs")
            self.notes["warm build"] = self.step("warm build", lambda: route.ssh(BUILD)).strip()
            self.step("end second", lambda: self.provider.end(second))
            live.remove(second)
            self.billed_seconds += time.monotonic() - second_start
        finally:
            for sandbox in live:
                self.provider.end(sandbox)
        print(json.dumps({
            "provider": self.provider.name,
            "vcpus": getattr(self.provider, "vcpus", VCPUS),
            "memory_gib": getattr(self.provider, "memory_gib", MEMORY_GIB),
            "steps_seconds": self.steps,
            "notes": self.notes,
            "billed_seconds_estimate": round(self.billed_seconds),
            "usd_estimate": round(self.provider.usd(self.billed_seconds), 3),
        }, indent=2))


PROVIDERS = {"modal": Modal, "daytona": Daytona}

if __name__ == "__main__":
    Trial(PROVIDERS[sys.argv[1]]()).run()
