import subprocess
import urllib.error
import urllib.request
from pathlib import Path

import pytest

from scripts import deploy_website


def _completed(stdout: str = "") -> subprocess.CompletedProcess[str]:
    return subprocess.CompletedProcess([], 0, stdout, "")


def test_health_probe_identifies_itself_to_production(monkeypatch: pytest.MonkeyPatch):
    class Response:
        status = 200

        def __init__(self, body: bytes):
            self._body = body

        def __enter__(self):
            return self

        def __exit__(self, *args: object) -> None:
            return None

        def read(self) -> bytes:
            return self._body

    def production(request: urllib.request.Request, timeout: int) -> Response:
        if request.get_header("User-agent") is None:
            raise urllib.error.HTTPError(request.full_url, 403, "Forbidden", {}, None)
        body = (
            b'{"status":"ok","release":"v1.2.3"}' if request.full_url.endswith("healthz") else b""
        )
        return Response(body)

    monkeypatch.setattr(deploy_website.urllib.request, "urlopen", production)

    assert deploy_website._release_is_healthy("v1.2.3")


def test_deploy_is_a_green_noop_when_the_tag_is_already_live(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
):
    receipts: list[deploy_website.DeployReceipt] = []
    monkeypatch.setenv("FLY_API_TOKEN", "injected-by-test")
    monkeypatch.setattr(deploy_website, "_verify_tag", lambda repo, tag: "abc123")
    monkeypatch.setattr(deploy_website, "_release_is_healthy", lambda tag: True)
    monkeypatch.setattr(
        deploy_website, "_write_receipt", lambda repo, receipt: receipts.append(receipt)
    )

    receipt = deploy_website.deploy_website("v1.2.3", tmp_path)

    assert receipt.outcome == "unchanged"
    assert receipts == [receipt]


def test_deploy_proves_the_exact_release_tag(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    images = iter(("registry/previous", "registry/deployed"))
    receipts: list[deploy_website.DeployReceipt] = []
    monkeypatch.setenv("FLY_API_TOKEN", "injected-by-test")
    monkeypatch.setattr(deploy_website, "_verify_tag", lambda repo, tag: "abc123")
    monkeypatch.setattr(deploy_website, "_release_is_healthy", lambda tag: False)
    monkeypatch.setattr(deploy_website, "_run", lambda *args, **kwargs: _completed())
    monkeypatch.setattr(deploy_website, "_current_image", lambda repo: next(images))
    monkeypatch.setattr(deploy_website, "_wait_for_release", lambda tag: tag == "v1.2.3")
    monkeypatch.setattr(
        deploy_website, "_write_receipt", lambda repo, receipt: receipts.append(receipt)
    )

    receipt = deploy_website.deploy_website("v1.2.3", tmp_path)

    assert receipt == deploy_website.DeployReceipt(
        tag="v1.2.3",
        source_commit="abc123",
        previous_image="registry/previous",
        deployed_image="registry/deployed",
        outcome="deployed",
    )
    assert receipts == [receipt]


def test_failed_release_proof_rolls_back_and_stays_red(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
):
    images = iter(("registry/previous", "registry/bad"))
    receipts: list[deploy_website.DeployReceipt] = []
    monkeypatch.setenv("FLY_API_TOKEN", "injected-by-test")
    monkeypatch.setattr(deploy_website, "_verify_tag", lambda repo, tag: "abc123")
    monkeypatch.setattr(deploy_website, "_release_is_healthy", lambda tag: False)
    monkeypatch.setattr(deploy_website, "_run", lambda *args, **kwargs: _completed())
    monkeypatch.setattr(deploy_website, "_current_image", lambda repo: next(images))
    monkeypatch.setattr(deploy_website, "_wait_for_release", lambda tag: False)
    monkeypatch.setattr(deploy_website, "_wait_for_root", lambda: True)
    monkeypatch.setattr(
        deploy_website, "_write_receipt", lambda repo, receipt: receipts.append(receipt)
    )

    with pytest.raises(RuntimeError, match="restored registry/previous"):
        deploy_website.deploy_website("v1.2.3", tmp_path)

    assert receipts == [
        deploy_website.DeployReceipt(
            tag="v1.2.3",
            source_commit="abc123",
            previous_image="registry/previous",
            deployed_image="registry/bad",
            outcome="rolled_back",
        )
    ]


def test_timed_out_fly_command_keeps_the_healthy_release(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
):
    images = iter(("registry/previous", "registry/deployed"))
    receipts: list[deploy_website.DeployReceipt] = []
    monkeypatch.setenv("FLY_API_TOKEN", "injected-by-test")
    monkeypatch.setattr(deploy_website, "_verify_tag", lambda repo, tag: "abc123")
    monkeypatch.setattr(deploy_website, "_release_is_healthy", lambda tag: False)
    monkeypatch.setattr(deploy_website, "_current_image", lambda repo: next(images))
    monkeypatch.setattr(deploy_website, "_wait_for_release", lambda tag: True)
    monkeypatch.setattr(
        deploy_website, "_write_receipt", lambda repo, receipt: receipts.append(receipt)
    )

    def time_out_new_deploy(
        command: list[str], cwd: Path, *, capture: bool = False
    ) -> subprocess.CompletedProcess[str]:
        if "--build-arg" in command:
            raise subprocess.CalledProcessError(1, command)
        return _completed()

    monkeypatch.setattr(deploy_website, "_run", time_out_new_deploy)

    receipt = deploy_website.deploy_website("v1.2.3", tmp_path)

    assert receipt == deploy_website.DeployReceipt(
        tag="v1.2.3",
        source_commit="abc123",
        previous_image="registry/previous",
        deployed_image="registry/deployed",
        outcome="deployed",
    )
    assert receipts == [receipt]


def test_timed_out_fly_command_restores_the_saved_image_when_unhealthy(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
):
    images = iter(("registry/previous", "registry/bad"))
    receipts: list[deploy_website.DeployReceipt] = []
    monkeypatch.setenv("FLY_API_TOKEN", "injected-by-test")
    monkeypatch.setattr(deploy_website, "_verify_tag", lambda repo, tag: "abc123")
    monkeypatch.setattr(deploy_website, "_release_is_healthy", lambda tag: False)
    monkeypatch.setattr(deploy_website, "_current_image", lambda repo: next(images))
    monkeypatch.setattr(deploy_website, "_wait_for_release", lambda tag: False)
    monkeypatch.setattr(deploy_website, "_wait_for_root", lambda: True)
    monkeypatch.setattr(
        deploy_website, "_write_receipt", lambda repo, receipt: receipts.append(receipt)
    )

    def fail_new_deploy(
        command: list[str], cwd: Path, *, capture: bool = False
    ) -> subprocess.CompletedProcess[str]:
        if "--build-arg" in command:
            raise subprocess.CalledProcessError(1, command)
        return _completed()

    monkeypatch.setattr(deploy_website, "_run", fail_new_deploy)

    with pytest.raises(RuntimeError, match="Fly deployment failed.*restored registry/previous"):
        deploy_website.deploy_website("v1.2.3", tmp_path)

    assert receipts == [
        deploy_website.DeployReceipt(
            tag="v1.2.3",
            source_commit="abc123",
            previous_image="registry/previous",
            deployed_image="registry/bad",
            outcome="rolled_back",
        )
    ]
