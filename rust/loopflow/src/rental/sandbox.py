"""Create, find and delete one rented sandbox on Daytona or Modal.

lf runs this as `python -c <this> <provider> <op>` inside
`uv run --no-project --with <sdk>`, with one JSON request on stdin, and reads
one JSON reply on stdout: the sandbox, `null` when it does not exist, or
`{"error": ...}`. The account key comes from the environment (Daytona) or
Modal's own config, and never appears in a request or a reply.

Every sandbox is named by lf before it is created, ends by itself when its
lifetime passes, and answers SSH: as `lf` on Modal, as root on Daytona.
"""

import json
import sys

# Shared by both images: what a Task needs on a fresh Linux box. Ubuntu 24.04
# carries the glibc the published lf is built against. Tools live system-wide
# because Daytona's SSH gateway logs in as root while Modal's sshd admits `lf`;
# IS_SANDBOX lets Claude Code skip permission prompts as root.
BASE = "ubuntu:24.04"
ENV = {
    "RUSTUP_HOME": "/usr/local/rustup",
    "CARGO_HOME": "/usr/local/cargo",
    "PATH": "/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    "IS_SANDBOX": "1",
}
TOOLS = [
    "apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq"
    " openssh-server git curl ca-certificates sudo build-essential pkg-config"
    " cmake clang libssl-dev python3",
    "curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg"
    " -o /usr/share/keyrings/githubcli-archive-keyring.gpg"
    " && echo 'deb [signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg]"
    " https://cli.github.com/packages stable main' > /etc/apt/sources.list.d/github-cli.list"
    " && apt-get update -qq && apt-get install -y -qq gh",
    "curl -fsSL https://deb.nodesource.com/setup_22.x | bash -"
    " && apt-get install -y -qq nodejs && npm install -g @anthropic-ai/claude-code",
    "curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path"
    " && chmod -R a+rwX /usr/local/rustup /usr/local/cargo",
    "curl -LsSf https://astral.sh/uv/install.sh | env UV_INSTALL_DIR=/usr/local/bin sh",
    "useradd -m -s /bin/bash lf && echo 'lf ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/lf",
    # sshd sessions read the environment from here, not from the image.
    "printf 'RUSTUP_HOME=/usr/local/rustup\\nCARGO_HOME=/usr/local/cargo\\nIS_SANDBOX=1\\n"
    "PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin\\n'"
    " > /etc/environment",
]

# Modal: key-only sshd for `lf`, in front of a TCP tunnel.
SSHD = """set -e
install -d -m 700 -o lf -g lf /home/lf/.ssh
printf '%s\\n' "$LF_PUBLIC_KEY" > /home/lf/.ssh/authorized_keys
chown lf:lf /home/lf/.ssh/authorized_keys && chmod 600 /home/lf/.ssh/authorized_keys
mkdir -p /run/sshd && ssh-keygen -A >/dev/null
exec /usr/sbin/sshd -D -e -o PasswordAuthentication=no -o KbdInteractiveAuthentication=no \
  -o PermitRootLogin=no -o AllowUsers=lf
"""

GONE_DAYTONA = {"destroyed", "destroying", "error", "build_failed"}


def _daytona(op, request):
    import daytona

    client = daytona.Daytona()

    def described(sandbox, route=None):
        state = str(getattr(sandbox.state, "value", sandbox.state)).lower()
        return {
            "id": sandbox.id,
            "name": sandbox.name,
            "gone": state in GONE_DAYTONA,
            "route": route,
        }

    if op == "create":
        image = daytona.Image.base(BASE).env(ENV).run_commands(*TOOLS)
        sandbox = client.create(
            daytona.CreateSandboxFromImageParams(
                name=request["name"],
                image=image,
                resources=daytona.Resources(
                    cpu=request["vcpus"],
                    memory=request["memory_gb"],
                    disk=request["disk_gb"],
                ),
                auto_stop_interval=0,
                ttl_minutes=request["lifetime_minutes"],
            ),
            timeout=1200,
        )
        access = sandbox.create_ssh_access(expires_in_minutes=request["lifetime_minutes"])
        return described(sandbox, f"ssh://{access.token}@ssh.app.daytona.io")
    if op == "get":
        # Daytona looks up an id or a name alike.
        try:
            return described(client.get(request.get("id") or request["name"]))
        except daytona.DaytonaNotFoundError:
            return None
    if op == "delete":
        try:
            client.delete(client.get(request["id"]))
        except daytona.DaytonaNotFoundError:
            pass
        return None
    raise ValueError(f"unknown op {op}")


def _modal(op, request):
    import modal

    app_name = "loopflow-rentals"

    def described(sandbox, route=None):
        return {
            "id": sandbox.object_id,
            "name": request.get("name"),
            "gone": sandbox.poll() is not None,
            "route": route,
        }

    if op == "create":
        app = modal.App.lookup(app_name, create_if_missing=True)
        image = (
            modal.Image.from_registry(BASE, add_python="3.12").env(ENV).run_commands(*TOOLS)
        )
        sandbox = modal.Sandbox.create(
            "bash", "-c", SSHD,
            app=app,
            name=request["name"],
            image=image,
            cpu=request["vcpus"] / 2,
            memory=request["memory_gb"] * 1024,
            timeout=request["lifetime_minutes"] * 60,
            unencrypted_ports=[22],
            env={"LF_PUBLIC_KEY": request["public_key"]},
        )
        host, port = sandbox.tunnels()[22].tcp_socket
        return described(sandbox, f"ssh://lf@{host}:{port}")
    if op == "get":
        try:
            if request.get("id"):
                return described(modal.Sandbox.from_id(request["id"]))
            # A name finds only a running sandbox; one that ended is gone either way.
            return described(modal.Sandbox.from_name(app_name, request["name"]))
        except modal.exception.NotFoundError:
            return None
    if op == "delete":
        try:
            modal.Sandbox.from_id(request["id"]).terminate()
        except modal.exception.NotFoundError:
            pass
        return None
    raise ValueError(f"unknown op {op}")


def main():
    provider, op = sys.argv[1], sys.argv[2]
    request = json.load(sys.stdin)
    try:
        reply = {"daytona": _daytona, "modal": _modal}[provider](op, request)
    except Exception as error:  # every failure goes back to lf as data
        reply = {"error": f"{type(error).__name__}: {error}"}
    json.dump(reply, sys.stdout)


main()
