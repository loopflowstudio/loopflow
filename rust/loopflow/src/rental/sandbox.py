"""Create, find and delete one rented machine on Daytona, Modal, RunPod, AWS or Fly.

lf runs this as `python -c <this> <provider> <op>` inside
`uv run --no-project [--with <sdk>]`, with one JSON request on stdin, and reads
one JSON reply on stdout: the machine, `null` when it does not exist, or
`{"error": ...}`. The account key comes from the environment (or Modal's own
config), and never appears in a request or a reply.

Every machine is named by lf before it is created, ends by itself when its
lifetime passes, and answers SSH: as root on Daytona, as `lf` elsewhere.
Daytona and Modal build an image with the tools; RunPod, AWS and Fly boot
stock Ubuntu 24.04 and install them before sshd admits `lf`, under the
guardian that holds the deadline (`guardian.py`, sent in each create).
"""

import base64
import json
import os
import shlex
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

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

# Admits `lf` with lf's key; until this runs, SSH as `lf` does not answer.
AUTHORIZE = """install -d -m 700 -o lf -g lf /home/lf/.ssh
printf '%s\\n' "$LF_PUBLIC_KEY" > /home/lf/.ssh/authorized_keys
chown lf:lf /home/lf/.ssh/authorized_keys && chmod 600 /home/lf/.ssh/authorized_keys
"""

# Key-only sshd for `lf` in the foreground: behind Modal's tunnel, RunPod's
# mapped port and Fly's IPv4 service.
SSHD = """mkdir -p /run/sshd && ssh-keygen -A >/dev/null
exec /usr/sbin/sshd -D -e -o PasswordAuthentication=no -o KbdInteractiveAuthentication=no \
  -o PermitRootLogin=no -o AllowUsers=lf
"""

# Stock Ubuntu: the image's environment and tools, then `lf` is admitted, so
# SSH as `lf` answers only once the tools are there.
SETUP = (
    "set -e\nexport DEBIAN_FRONTEND=noninteractive\n"
    + "".join(f"export {key}={shlex.quote(value)}\n" for key, value in ENV.items())
    + "\n".join(TOOLS)
    + "\n"
    + AUTHORIZE
)

# RunPod and Fly containers: the guardian is the main process and the setup
# its child, so the container ends when either does. Without python3 the
# deadline still holds through `timeout`.
CONTAINER_BOOT = """set -u
mkdir -p /opt/lf
printf '%s' "$LF_GUARDIAN" | base64 -d > /opt/lf/guardian.py
unset LF_GUARDIAN
command -v python3 >/dev/null || timeout 600 sh -c 'apt-get update -qq && \
  DEBIAN_FRONTEND=noninteractive apt-get install -y -qq python3 ca-certificates' \
  >/var/log/lf-python.log 2>&1
if command -v python3 >/dev/null; then
  exec python3 /opt/lf/guardian.py -- bash -c "$LF_SETUP"
fi
exec timeout "$LF_SECONDS" bash -c "$LF_SETUP"
"""


def _guardian_env(request):
    return {
        "LF_DEADLINE": request["deadline"],
        "LF_CREATED": request["created_at"],
        "LF_MAX_USD": request["max_usd"],
    }


def _container_env(request):
    """A container's environment: its guardian, its setup and lf's key."""
    return {
        **_guardian_env(request),
        "LF_GUARDIAN": base64.b64encode(request["guardian"].encode()).decode(),
        "LF_SETUP": SETUP + SSHD,
        "LF_SECONDS": str(request["lifetime_minutes"] * 60),
        "LF_PUBLIC_KEY": request["public_key"],
    }


def _usd(value):
    """A provider's hourly price as exact decimal text."""
    return None if value is None else f"{float(value):.6f}"


def _http(method, url, authorization, body=None):
    """One JSON request: the reply, `None` for 404, an exception otherwise."""
    request = urllib.request.Request(
        url,
        data=None if body is None else json.dumps(body).encode(),
        method=method,
        headers={
            "Authorization": authorization,
            "Content-Type": "application/json",
            # RunPod's front door refuses default agent names.
            "User-Agent": "loopflow",
        },
    )
    try:
        with urllib.request.urlopen(request, timeout=60) as reply:  # noqa: S310
            raw = reply.read()
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        detail = error.read(300).decode(errors="replace")
        raise RuntimeError(f"{method} {url.split('?')[0]}: status {error.code} {detail}") from None
    return json.loads(raw) if raw.strip() else {}


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
        image = modal.Image.from_registry(BASE, add_python="3.12").env(ENV).run_commands(*TOOLS)
        sandbox = modal.Sandbox.create(
            "bash",
            "-c",
            "set -e\n" + AUTHORIZE + SSHD,
            app=app,
            name=request["name"],
            image=image,
            cpu=request["vcpus"] / 2,
            memory=request["memory_gb"] * 1024,
            timeout=request["lifetime_minutes"] * 60,
            unencrypted_ports=[22],
            env={"LF_PUBLIC_KEY": request["public_key"]},
            gpu=request["type"],
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


# ---------------------------------------------------------------------------
# RunPod: a pod from stock Ubuntu, reached on its public IP and mapped port 22.
# ---------------------------------------------------------------------------

RUNPOD = "https://rest.runpod.io/v1/"


def _runpod(op, request):
    authorization = "Bearer " + os.environ["RUNPOD_API_KEY"]

    def ask(method, path, body=None):
        return _http(method, RUNPOD + path, authorization, body)

    def described(pod, route=None):
        return {
            "id": pod["id"],
            "gone": pod.get("desiredStatus") in ("EXITED", "TERMINATED"),
            "route": route,
            "usd_per_hour": _usd(pod.get("costPerHr")),
        }

    if op == "create":
        body = {
            "name": request["name"],
            "imageName": BASE,
            "cloudType": "SECURE",
            "containerDiskInGb": request["disk_gb"],
            "ports": ["22/tcp"],
            "dockerEntrypoint": ["bash", "-c", CONTAINER_BOOT],
            "dockerStartCmd": [],
            "env": _container_env(request),
        }
        if request["gpu"]:
            # --vcpus and --memory-gb are floors the GPU's machine must meet.
            body.update(
                computeType="GPU",
                gpuTypeIds=[request["type"]],
                gpuCount=1,
                minVCPUPerGPU=request["vcpus"],
                minRAMPerGPU=request["memory_gb"],
            )
        else:
            body.update(
                computeType="CPU", cpuFlavorIds=[request["type"]], vcpuCount=request["vcpus"]
            )
        if request["spot"]:
            # RunPod bids the interruptible price; the guardian rereads what
            # it charges, which never exceeds the admitted on-demand price.
            body["interruptible"] = True
        pod = ask("POST", "pods", body)
        # RunPod assigns the public address and port after the create answers.
        for _ in range(120):
            port = (pod.get("portMappings") or {}).get("22")
            if pod.get("publicIp") and port:
                return described(pod, f"ssh://lf@{pod['publicIp']}:{port}")
            if described(pod)["gone"]:
                break
            time.sleep(5)
            pod = ask("GET", f"pods/{pod['id']}") or pod
        raise RuntimeError(f"pod {pod['id']} has no public SSH port")
    if op == "get":
        if request.get("id"):
            pod = ask("GET", f"pods/{request['id']}")
            return pod and described(pod)
        pods = ask("GET", "pods?" + urllib.parse.urlencode({"name": request["name"]})) or []
        named = [pod for pod in pods if pod.get("name") == request["name"]]
        return described(named[0]) if named else None
    if op == "delete":
        ask("DELETE", f"pods/{request['id']}")
        return None
    raise ValueError(f"unknown op {op}")


# ---------------------------------------------------------------------------
# AWS: an EC2 instance from Canonical's Ubuntu 24.04 AMI, tagged with its name.
# ---------------------------------------------------------------------------

AWS_TAG = "loopflow:name"
AWS_GROUP = "loopflow-rentals"
AWS_AMI = "/aws/service/canonical/ubuntu/server/24.04/stable/current/amd64/hvm/ebs-gp3/ami-id"
AWS_GONE = {"shutting-down", "terminated", "stopping", "stopped"}
AWS_LIVE = ["pending", "running", "shutting-down", "stopping", "stopped"]


def _aws_code(error):
    return getattr(error, "response", {}).get("Error", {}).get("Code", "")


def _aws_user_data(request):
    """Cloud-init: the guardian powers the instance off at the deadline, which
    terminates it; then the tools, then `lf` is admitted to the stock sshd."""
    guardian = base64.b64encode(request["guardian"].encode()).decode()
    setenv = " ".join(
        shlex.quote(f"--setenv={key}={value}") for key, value in _guardian_env(request).items()
    )
    return (
        "#!/bin/bash\n"
        "mkdir -p /opt/lf\n"
        f"echo {guardian} | base64 -d > /opt/lf/guardian.py\n"
        f"systemd-run --unit lf-guardian {setenv} "
        "sh -c 'python3 /opt/lf/guardian.py; shutdown -h now'\n"
        f"export LF_PUBLIC_KEY={shlex.quote(request['public_key'])}\n" + SETUP
    )


def _aws_ssh_group(ec2):
    """The security group admitting SSH, created once per account and region."""
    found = ec2.describe_security_groups(Filters=[{"Name": "group-name", "Values": [AWS_GROUP]}])
    if found["SecurityGroups"]:
        return found["SecurityGroups"][0]["GroupId"]
    try:
        group = ec2.create_security_group(
            GroupName=AWS_GROUP, Description="SSH to machines Loopflow rents"
        )["GroupId"]
    except Exception as error:
        if _aws_code(error) != "InvalidGroup.Duplicate":
            raise
        return _aws_ssh_group(ec2)
    ec2.authorize_security_group_ingress(
        GroupId=group,
        IpPermissions=[
            {
                "IpProtocol": "tcp",
                "FromPort": 22,
                "ToPort": 22,
                "IpRanges": [{"CidrIp": "0.0.0.0/0"}],
            }
        ],
    )
    return group


def _aws(op, request):
    import boto3

    ec2 = boto3.client("ec2")

    def described(instance, route=None):
        return {
            "id": instance["InstanceId"],
            "gone": instance["State"]["Name"] in AWS_GONE,
            "route": route,
        }

    def instances(**filters):
        reply = ec2.describe_instances(**filters)
        return [i for reservation in reply["Reservations"] for i in reservation["Instances"]]

    if op == "create":
        ami = boto3.client("ssm").get_parameter(Name=AWS_AMI)["Parameter"]["Value"]
        name = request["name"]
        spot = {}
        if request["spot"]:
            # Never charged above the admitted price, so the budget holds.
            spot["InstanceMarketOptions"] = {
                "MarketType": "spot",
                "SpotOptions": {
                    "MaxPrice": request["usd_per_hour"],
                    "SpotInstanceType": "one-time",
                    "InstanceInterruptionBehavior": "terminate",
                },
            }
        instance = ec2.run_instances(
            **spot,
            ImageId=ami,
            InstanceType=request["type"],
            MinCount=1,
            MaxCount=1,
            SecurityGroupIds=[_aws_ssh_group(ec2)],
            UserData=_aws_user_data(request),
            InstanceInitiatedShutdownBehavior="terminate",
            BlockDeviceMappings=[
                {
                    "DeviceName": "/dev/sda1",
                    "Ebs": {
                        "VolumeSize": request["disk_gb"],
                        "VolumeType": "gp3",
                        "DeleteOnTermination": True,
                    },
                }
            ],
            TagSpecifications=[
                {
                    "ResourceType": "instance",
                    "Tags": [{"Key": "Name", "Value": name}, {"Key": AWS_TAG, "Value": name}],
                }
            ],
            # A resent create returns the same instance instead of a second one.
            ClientToken=name,
        )["Instances"][0]
        ec2.get_waiter("instance_running").wait(InstanceIds=[instance["InstanceId"]])
        instance = instances(InstanceIds=[instance["InstanceId"]])[0]
        if not instance.get("PublicIpAddress"):
            raise RuntimeError(f"{instance['InstanceId']} has no public IP")
        return described(instance, f"ssh://lf@{instance['PublicIpAddress']}")
    if op == "get":
        try:
            if request.get("id"):
                found = instances(InstanceIds=[request["id"]])
            else:
                found = instances(
                    Filters=[
                        {"Name": f"tag:{AWS_TAG}", "Values": [request["name"]]},
                        {"Name": "instance-state-name", "Values": AWS_LIVE},
                    ]
                )
        except Exception as error:
            if _aws_code(error) == "InvalidInstanceID.NotFound":
                return None
            raise
        return described(found[0]) if found else None
    if op == "delete":
        try:
            ec2.terminate_instances(InstanceIds=[request["id"]])
        except Exception as error:
            if _aws_code(error) != "InvalidInstanceID.NotFound":
                raise
        return None
    raise ValueError(f"unknown op {op}")


# ---------------------------------------------------------------------------
# Fly: one app per rental, holding one Machine and a dedicated IPv4 for SSH.
# Fly's proxy balances across an app's Machines, so each needs its own app.
# The app's name is the rental's, and it is the provider id.
# ---------------------------------------------------------------------------

FLY_MACHINES = "https://api.machines.dev/v1/"
FLY_GRAPHQL = "https://api.fly.io/graphql"
FLY_GONE = {"stopping", "stopped", "suspended", "destroying", "destroyed", "failed"}
FLY_IPV4 = """mutation($input: AllocateIPAddressInput!) {
  allocateIpAddress(input: $input) { ipAddress { address } }
}"""


def _fly_app(name):
    """Fly app names take lowercase letters, digits and dashes."""
    return name.lower().replace("_", "-")


def _fly(op, request):
    token = os.environ["FLY_API_TOKEN"]
    authorization = token if token.startswith("FlyV1 ") else f"Bearer {token}"

    def ask(method, path, body=None):
        return _http(method, FLY_MACHINES + path, authorization, body)

    def found(app):
        machines = ask("GET", f"apps/{app}/machines")
        if machines is None:
            return None
        # An app whose Machine ended still holds its IPv4 until deleted.
        gone = all(machine.get("state") in FLY_GONE for machine in machines)
        return {"id": app, "gone": gone, "route": None}

    if op == "create":
        app = _fly_app(request["name"])
        ask("POST", "apps", {"app_name": app, "org_slug": os.environ.get("FLY_ORG", "personal")})
        reply = _http(
            "POST",
            FLY_GRAPHQL,
            authorization,
            {"query": FLY_IPV4, "variables": {"input": {"appId": app, "type": "v4"}}},
        )
        if reply.get("errors"):
            raise RuntimeError(f"IPv4 for {app}: {reply['errors'][0].get('message')}")
        address = reply["data"]["allocateIpAddress"]["ipAddress"]["address"]
        ask(
            "POST",
            f"apps/{app}/machines",
            {
                "name": app,
                "config": {
                    "image": "registry-1.docker.io/library/" + BASE,
                    "guest": {
                        "cpu_kind": "performance",
                        "cpus": request["vcpus"],
                        "memory_mb": request["memory_gb"] * 1024,
                    },
                    "init": {"entrypoint": ["bash", "-c", CONTAINER_BOOT]},
                    "env": _container_env(request),
                    "services": [
                        {
                            "protocol": "tcp",
                            "internal_port": 22,
                            "ports": [{"port": 22}],
                            "autostop": "off",
                            "autostart": False,
                        }
                    ],
                    # When the guardian exits, the Machine is deleted.
                    "auto_destroy": True,
                    "restart": {"policy": "no"},
                },
            },
        )
        return {"id": app, "gone": False, "route": f"ssh://lf@{address}"}
    if op == "get":
        return found(request.get("id") or _fly_app(request["name"]))
    if op == "delete":
        # Deleting the app deletes its Machine and releases its IPv4.
        ask("DELETE", f"apps/{request['id']}?force=true")
        return None
    raise ValueError(f"unknown op {op}")


PROVIDERS = {
    "daytona": _daytona,
    "modal": _modal,
    "runpod": _runpod,
    "aws": _aws,
    "fly": _fly,
}


def main():
    provider, op = sys.argv[1], sys.argv[2]
    request = json.load(sys.stdin)
    try:
        reply = PROVIDERS[provider](op, request)
    except Exception as error:  # every failure goes back to lf as data
        reply = {"error": f"{type(error).__name__}: {error}"}
    json.dump(reply, sys.stdout)


if __name__ == "__main__":
    main()
