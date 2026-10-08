import runpy
import sqlite3
from pathlib import Path


def test_dense_fixture_keeps_source_and_history_but_drops_external_account_routes(tmp_path):
    prepare = runpy.run_path(
        str(Path(__file__).resolve().parents[2] / "scripts/benchmarks/agent-startup/fixture.py")
    )["prepare"]
    source = tmp_path / "installed.db"
    with sqlite3.connect(source) as db:
        db.executescript("""
            PRAGMA foreign_keys=ON;
            CREATE TABLE provider_accounts(id TEXT PRIMARY KEY, home TEXT);
            CREATE TABLE provider_routes(account TEXT REFERENCES provider_accounts(id));
            CREATE TABLE provider_session_accounts(account TEXT REFERENCES provider_accounts(id));
            CREATE TABLE auth_browser_bindings(account TEXT REFERENCES provider_accounts(id));
            CREATE TABLE agent_sessions(id TEXT);
            CREATE TABLE processes(id TEXT);
            CREATE TABLE session_events(payload TEXT);
            INSERT INTO provider_accounts VALUES('account', '/outside/provider-home');
            INSERT INTO provider_routes VALUES('account');
            INSERT INTO provider_session_accounts VALUES('account');
            INSERT INTO auth_browser_bindings VALUES('account');
            INSERT INTO agent_sessions VALUES('retained-session');
            INSERT INTO processes VALUES('retained-process');
            INSERT INTO session_events VALUES('retained-history');
        """)
    before = source.read_bytes()
    home = tmp_path / "fixture"
    receipt = prepare(source, home, False)
    assert source.read_bytes() == before
    assert receipt["managed_account_routes_removed"]
    with sqlite3.connect(home / "loopflow.db") as db:
        for table in (
            "provider_accounts",
            "provider_routes",
            "provider_session_accounts",
            "auth_browser_bindings",
        ):
            assert db.execute(f"SELECT count(*) FROM {table}").fetchone() == (0,)
        assert db.execute("SELECT id FROM agent_sessions").fetchone() == ("retained-session",)
        assert db.execute("SELECT id FROM processes").fetchone() == ("retained-process",)
        assert db.execute("SELECT payload FROM session_events").fetchone() == ("retained-history",)
        assert db.execute("PRAGMA foreign_key_check").fetchall() == []
