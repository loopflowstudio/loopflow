import asyncio

from loopflow.lfd.daemon.machine_id import get_machine_id


def test_machine_id_persists(tmp_path, monkeypatch):
    from loopflow.lfd.daemon import machine_id as machine_id_module

    monkeypatch.setattr(machine_id_module.Path, "home", lambda: tmp_path)

    first = get_machine_id()
    second = get_machine_id()

    assert first == second
    stored = (tmp_path / ".lf" / "machine_id").read_text().strip()
    assert stored == first


def test_connection_validator_uses_cache(monkeypatch):
    from loopflow.lfd.daemon import connection_validator

    now = 1_700_000_000.0
    monkeypatch.setattr(connection_validator.time, "time", lambda: now)

    async def fake_post_json(url, payload, headers=None, timeout=5):
        return 200, {"valid": True, "expires_at": now + 60}

    monkeypatch.setattr(connection_validator, "_post_json", fake_post_json)

    validator = connection_validator.ConnectionValidator(base_url="https://example.com")
    assert asyncio.run(validator.validate_connection_token("token")) is True

    monkeypatch.setattr(connection_validator.time, "time", lambda: now + 10)

    async def fail_post_json(url, payload, headers=None, timeout=5):
        raise AssertionError("should use cached token result")

    monkeypatch.setattr(connection_validator, "_post_json", fail_post_json)

    assert asyncio.run(validator.validate_connection_token("token")) is True
