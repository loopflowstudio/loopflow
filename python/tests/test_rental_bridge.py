"""The rental bridge's requests to providers, over mocked HTTP and SDKs."""

import base64
import importlib.util
import io
import json
import shlex
import subprocess
import sys
import types
import urllib.error
from pathlib import Path
from unittest.mock import MagicMock

import pytest

BRIDGE = Path(__file__).parents[2] / "rust/loopflow-execution/src/rental/sandbox.py"
_spec = importlib.util.spec_from_file_location("sandbox", BRIDGE)
bridge = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(bridge)

REQUEST = {
    "name": "lf-rent_0b5c",
    "vcpus": 4,
    "memory_gb": 16,
    "disk_gb": 40,
    "gpu": None,
    "spot": False,
    "type": "cpu3g",
    "usd_per_hour": "0.16",
    "lifetime_minutes": 120,
    "public_key": "ssh-ed25519 AAAA lf",
    "deadline": "2026-10-26T22:00:00Z",
    "created_at": "2026-10-26T20:00:00Z",
    "max_usd": "1.50",
    "guardian": "print('guard')\n",
}


class Http:
    """Answers each request with the next reply whose method and path match."""

    def __init__(self, routes):
        self.routes = routes
        self.sent = []

    def __call__(self, request, timeout):
        body = json.loads(request.data) if request.data else None
        self.sent.append((request.get_method(), request.full_url, body, dict(request.headers)))
        for (method, fragment), replies in self.routes.items():
            if method == request.get_method() and fragment in request.full_url and replies:
                status, reply = replies.pop(0) if len(replies) > 1 else replies[0]
                if status >= 400:
                    raise urllib.error.HTTPError(
                        request.full_url, status, "", {}, io.BytesIO(b"nope")
                    )
                return io.BytesIO(json.dumps(reply).encode())
        raise AssertionError(f"unexpected {request.get_method()} {request.full_url}")


@pytest.fixture
def http(monkeypatch):
    def install(routes):
        fake = Http(routes)
        monkeypatch.setattr(bridge.urllib.request, "urlopen", fake)
        monkeypatch.setattr(bridge.time, "sleep", lambda _: None)
        return fake

    monkeypatch.setenv("RUNPOD_API_KEY", "rp-key")
    return install


# --- RunPod ---------------------------------------------------------------


def test_runpod_creates_a_cpu_pod_and_waits_for_its_ssh_port(http):
    fake = http(
        {
            ("POST", "/pods"): [(201, {"id": "p1", "desiredStatus": "RUNNING", "costPerHr": 0.16})],
            ("GET", "/pods/p1"): [
                (200, {"id": "p1", "desiredStatus": "RUNNING", "publicIp": ""}),
                (
                    200,
                    {
                        "id": "p1",
                        "desiredStatus": "RUNNING",
                        "publicIp": "203.0.113.7",
                        "portMappings": {"22": 40122},
                        "costPerHr": 0.16,
                    },
                ),
            ],
        }
    )
    reply = bridge._runpod("create", REQUEST)
    assert reply == {
        "id": "p1",
        "gone": False,
        "route": "ssh://lf@203.0.113.7:40122",
        "usd_per_hour": "0.160000",
    }
    method, url, body, headers = fake.sent[0]
    assert (method, url) == ("POST", "https://rest.runpod.io/v1/pods")
    assert headers["Authorization"] == "Bearer rp-key"
    assert body["name"] == "lf-rent_0b5c"
    assert (body["computeType"], body["cpuFlavorIds"], body["vcpuCount"]) == ("CPU", ["cpu3g"], 4)
    assert body["ports"] == ["22/tcp"]
    env = body["env"]
    assert base64.b64decode(env["LF_GUARDIAN"]).decode() == REQUEST["guardian"]
    assert env["LF_DEADLINE"] == REQUEST["deadline"] and env["LF_MAX_USD"] == "1.50"
    # The account key never enters the pod; RunPod injects a pod-scoped one.
    assert "rp-key" not in json.dumps(body)


def test_runpod_asks_for_one_gpu_by_its_type(http):
    fake = http(
        {
            ("POST", "/pods"): [
                (
                    201,
                    {
                        "id": "p2",
                        "publicIp": "203.0.113.8",
                        "portMappings": {"22": 40000},
                        "costPerHr": 0.59,
                    },
                )
            ]
        }
    )
    bridge._runpod("create", {**REQUEST, "gpu": "A40", "type": "NVIDIA A40"})
    body = fake.sent[0][2]
    assert (body["computeType"], body["gpuTypeIds"], body["gpuCount"]) == (
        "GPU",
        ["NVIDIA A40"],
        1,
    )
    assert "cpuFlavorIds" not in body
    # The GPU's machine must meet --vcpus and --memory-gb.
    assert (body["minVCPUPerGPU"], body["minRAMPerGPU"]) == (4, 16)
    assert "interruptible" not in body


def test_runpod_rents_spot_as_an_interruptible_pod(http):
    pod = {"id": "p3", "publicIp": "203.0.113.9", "portMappings": {"22": 40001}}
    fake = http({("POST", "/pods"): [(201, pod)]})
    bridge._runpod("create", {**REQUEST, "spot": True})
    assert fake.sent[0][2]["interruptible"] is True


def test_runpod_finds_a_pod_by_name_and_reads_absence(http):
    fake = http(
        {
            ("GET", "/pods?name="): [(200, [{"id": "p1", "name": "lf-rent_0b5c"}])],
            ("GET", "/pods/gone"): [(404, None)],
        }
    )
    assert bridge._runpod("get", {"name": "lf-rent_0b5c"})["id"] == "p1"
    assert bridge._runpod("get", {"id": "gone"}) is None
    assert "name=lf-rent_0b5c" in fake.sent[0][1]


def test_runpod_unavailable_is_not_absent(http):
    http({("GET", "/pods/p1"): [(503, None)]})
    with pytest.raises(RuntimeError, match="503"):
        bridge._runpod("get", {"id": "p1"})


# --- AWS ------------------------------------------------------------------


class Ec2:
    def __init__(self, groups):
        self.groups = groups
        self.calls = []

    def __getattr__(self, name):
        def call(*args, **kwargs):
            self.calls.append((name, kwargs))
            if name == "describe_security_groups":
                return {"SecurityGroups": self.groups}
            if name == "create_security_group":
                return {"GroupId": "sg-new"}
            if name == "run_instances":
                return {"Instances": [{"InstanceId": "i-1"}]}
            if name == "describe_instances":
                instance = {
                    "InstanceId": "i-1",
                    "State": {"Name": "running"},
                    "PublicIpAddress": "198.51.100.4",
                }
                return {"Reservations": [{"Instances": [instance]}]}
            if name == "get_waiter":
                return types.SimpleNamespace(wait=lambda **_: None)
            return {}

        return call


@pytest.fixture
def aws(monkeypatch):
    ec2 = Ec2(groups=[])
    ssm = types.SimpleNamespace(get_parameter=lambda Name: {"Parameter": {"Value": "ami-123"}})
    ec2.regions = []

    def client(service, region_name):
        ec2.regions.append(region_name)
        return ec2 if service == "ec2" else ssm

    monkeypatch.setitem(sys.modules, "boto3", types.SimpleNamespace(client=client))
    monkeypatch.setenv("AWS_REGION", "us-east-1")
    return ec2


def test_aws_creates_a_tagged_instance_that_terminates_itself(aws):
    reply = bridge._aws("create", {**REQUEST, "type": "m7i.xlarge"})
    assert reply == {"id": "i-1", "gone": False, "route": "ssh://lf@198.51.100.4"}
    run = dict(aws.calls)["run_instances"]
    assert run["ImageId"] == "ami-123" and run["InstanceType"] == "m7i.xlarge"
    assert run["InstanceInitiatedShutdownBehavior"] == "terminate"
    assert "InstanceMarketOptions" not in run
    assert run["ClientToken"] == "lf-rent_0b5c"
    assert {"Key": "loopflow:name", "Value": "lf-rent_0b5c"} in run["TagSpecifications"][0]["Tags"]
    assert run["SecurityGroupIds"] == ["sg-new"]
    user_data = run["UserData"]
    assert "--setenv=LF_DEADLINE=2026-10-26T22:00:00Z" in user_data
    assert "python3 /opt/lf/guardian.py; shutdown -h now" in user_data
    # SSH as lf answers only after the tools are installed.
    assert user_data.index("claude-code") < user_data.index("authorized_keys")
    ingress = dict(aws.calls)["authorize_security_group_ingress"]
    assert ingress["IpPermissions"][0]["FromPort"] == 22


def test_aws_rents_in_the_region_lf_quoted(aws, monkeypatch):
    monkeypatch.setenv("AWS_DEFAULT_REGION", "us-west-2")
    bridge._aws("create", {**REQUEST, "type": "m7i.xlarge"})
    assert set(aws.regions) == {"us-east-1"}


def test_aws_rents_spot_capped_at_the_admitted_price(aws):
    bridge._aws("create", {**REQUEST, "type": "m7i.xlarge", "spot": True})
    market = dict(aws.calls)["run_instances"]["InstanceMarketOptions"]
    assert market["MarketType"] == "spot"
    assert market["SpotOptions"]["MaxPrice"] == "0.16"
    assert market["SpotOptions"]["InstanceInterruptionBehavior"] == "terminate"


def test_aws_finds_a_live_instance_by_its_tag(aws):
    assert bridge._aws("get", {"name": "lf-rent_0b5c"})["id"] == "i-1"
    filters = dict(aws.calls)["describe_instances"]["Filters"]
    assert {"Name": "tag:loopflow:name", "Values": ["lf-rent_0b5c"]} in filters


def test_aws_reuses_its_security_group(aws):
    aws.groups = [{"GroupId": "sg-old"}]
    bridge._aws("create", {**REQUEST, "type": "c7i.large"})
    assert dict(aws.calls)["run_instances"]["SecurityGroupIds"] == ["sg-old"]
    assert "create_security_group" not in dict(aws.calls)


# --- Daytona --------------------------------------------------------------


@pytest.mark.parametrize("state, deletes", [("started", True), ("destroyed", False)])
def test_daytona_deletes_only_a_sandbox_not_yet_destroyed(monkeypatch, state, deletes):
    deleted = []
    sandbox = types.SimpleNamespace(id="sb-1", name="lf-rent_0b5c", state=state)
    client = types.SimpleNamespace(get=lambda _: sandbox, delete=deleted.append)
    daytona = types.SimpleNamespace(Daytona=lambda: client, DaytonaNotFoundError=LookupError)
    monkeypatch.setitem(sys.modules, "daytona", daytona)
    assert bridge._daytona("delete", {"id": "sb-1"}) is None
    assert bool(deleted) == deletes


# --- Modal ----------------------------------------------------------------


@pytest.mark.parametrize(
    "mode,cpu,memory",
    [("burst", (0.125, 4.0), (128, 16384)), ("reserved", (4.0, 4.0), (16384, 16384))],
)
def test_modal_caps_both_resources_and_stops_at_the_admitted_deadline(
    monkeypatch, mode, cpu, memory
):
    modal = MagicMock()
    monkeypatch.setitem(sys.modules, "modal", modal)
    sandbox = modal.Sandbox.create.return_value
    sandbox.poll.return_value = None
    sandbox.tunnels.return_value = {22: types.SimpleNamespace(tcp_socket=("modal.test", 4321))}
    monkeypatch.setattr(bridge.time, "time", lambda: 100.5)
    reply = bridge._modal(
        "create",
        {
            **REQUEST,
            "vcpus": 8,
            "modal_sizing": mode,
            "type": "L4",
            "deadline": "1970-01-01T00:02:00Z",
        },
    )
    args = modal.Sandbox.create.call_args.kwargs
    assert (args["cpu"], args["memory"], args["gpu"], args["timeout"]) == (cpu, memory, "L4", 19)
    assert reply["route"] == "ssh://lf@modal.test:4321"


def test_non_login_ssh_finds_the_uploaded_cli_and_provider_tools(tmp_path):
    environment = tmp_path / "environment"
    for command in bridge.TOOLS:
        if "/etc/environment" in command:
            subprocess.run(
                ["sh", "-ec", command.replace("/etc/environment", shlex.quote(str(environment)))],
                check=True,
            )
    settings = dict(line.split("=", 1) for line in environment.read_text().splitlines())
    assert {"/root/.local/bin", "/home/lf/.local/bin"} <= set(settings["PATH"].split(":"))
    packages = next(
        command.split("npm install -g ")[1].split()
        for command in bridge.TOOLS
        if "npm install -g " in command
    )
    assert {"@openai/codex", "@anthropic-ai/claude-code"} <= set(packages)


def test_modal_skips_create_when_the_budget_window_has_elapsed(monkeypatch):
    modal = MagicMock()
    monkeypatch.setitem(sys.modules, "modal", modal)
    monkeypatch.setattr(bridge.time, "time", lambda: 121)
    with pytest.raises(ValueError, match="deadline passed"):
        bridge._modal(
            "create", {**REQUEST, "modal_sizing": "burst", "deadline": "1970-01-01T00:02:00Z"}
        )
    modal.Sandbox.create.assert_not_called()
