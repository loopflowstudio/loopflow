"""The rental bridge's requests to RunPod, AWS and Fly, over mocked HTTP and SDKs."""

import base64
import importlib.util
import io
import json
import sys
import types
import urllib.error
from pathlib import Path

import pytest

BRIDGE = Path(__file__).parents[2] / "rust/loopflow/src/rental/sandbox.py"
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
    monkeypatch.setenv("FLY_API_TOKEN", "FlyV1 fm2_token")
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
    boto3 = types.SimpleNamespace(client=lambda service: ec2 if service == "ec2" else ssm)
    monkeypatch.setitem(sys.modules, "boto3", boto3)
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


# --- Fly ------------------------------------------------------------------


def test_fly_creates_an_app_with_ipv4_and_a_self_destroying_machine(http):
    fake = http(
        {
            ("POST", "/v1/apps/lf-rent-0b5c/machines"): [(200, {"id": "m1"})],
            ("POST", "/v1/apps"): [(201, {"id": "app1"})],
            ("POST", "/graphql"): [
                (200, {"data": {"allocateIpAddress": {"ipAddress": {"address": "192.0.2.9"}}}})
            ],
        }
    )
    reply = bridge._fly("create", REQUEST)
    assert reply == {"id": "lf-rent-0b5c", "gone": False, "route": "ssh://lf@192.0.2.9"}
    app, ip, machine = (sent[2] for sent in fake.sent)
    assert app == {"app_name": "lf-rent-0b5c", "org_slug": "personal"}
    assert ip["variables"]["input"] == {"appId": "lf-rent-0b5c", "type": "v4"}
    config = machine["config"]
    assert config["guest"] == {"cpu_kind": "performance", "cpus": 4, "memory_mb": 16384}
    assert config["auto_destroy"] is True and config["restart"] == {"policy": "no"}
    assert config["services"][0]["internal_port"] == 22
    assert fake.sent[0][3]["Authorization"] == "FlyV1 fm2_token"


def test_fly_reads_an_ended_machine_as_gone_and_a_missing_app_as_absent(http):
    http(
        {
            ("GET", "/apps/lf-rent-0b5c/machines"): [(200, [{"id": "m1", "state": "destroyed"}])],
            ("GET", "/apps/lf-rent-none/machines"): [(404, None)],
        }
    )
    assert bridge._fly("get", {"name": "lf-rent_0b5c"}) == {
        "id": "lf-rent-0b5c",
        "gone": True,
        "route": None,
    }
    assert bridge._fly("get", {"id": "lf-rent-none"}) is None


def test_fly_delete_removes_the_app(http):
    fake = http({("DELETE", "/apps/lf-rent-0b5c"): [(202, {})]})
    bridge._fly("delete", {"id": "lf-rent-0b5c"})
    assert fake.sent[0][1].endswith("/v1/apps/lf-rent-0b5c?force=true")
