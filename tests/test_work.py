"""Tests for work queue functionality."""

from pathlib import Path
import tempfile

from loopflow.work.models import WorkItem, get_next_work
from loopflow.work.file_backend import FileBackend


def test_work_item_defaults():
    item = WorkItem(id="test", title="Test", description="A test item")
    assert item.status == "proposed"
    assert item.claimed_by is None
    assert item.blocked_on is None


def test_get_next_work_picks_approved():
    items = [
        WorkItem(id="a", title="A", description="", status="proposed"),
        WorkItem(id="b", title="B", description="", status="approved"),
        WorkItem(id="c", title="C", description="", status="done"),
    ]
    result = get_next_work(items)
    assert result is not None
    assert result.id == "b"


def test_get_next_work_skips_human_claimed():
    items = [
        WorkItem(id="a", title="A", description="", status="approved", claimed_by="human"),
        WorkItem(id="b", title="B", description="", status="approved"),
    ]
    result = get_next_work(items)
    assert result is not None
    assert result.id == "b"


def test_get_next_work_prefers_non_blocked():
    items = [
        WorkItem(id="a", title="A", description="", status="approved", blocked_on="waiting for input"),
        WorkItem(id="b", title="B", description="", status="approved"),
    ]
    result = get_next_work(items)
    assert result is not None
    assert result.id == "b"


def test_get_next_work_returns_none_when_empty():
    result = get_next_work([])
    assert result is None


def test_get_next_work_returns_none_when_no_approved():
    items = [
        WorkItem(id="a", title="A", description="", status="proposed"),
        WorkItem(id="b", title="B", description="", status="done"),
    ]
    result = get_next_work(items)
    assert result is None


def test_file_backend_create_and_list():
    with tempfile.TemporaryDirectory() as tmpdir:
        backend = FileBackend(Path(tmpdir))
        item = WorkItem(id="", title="Test Item", description="A test")
        created = backend.create_item(item)

        assert created.id == "test-item"

        items = backend.list_items()
        assert len(items) == 1
        assert items[0].title == "Test Item"


def test_file_backend_update():
    with tempfile.TemporaryDirectory() as tmpdir:
        backend = FileBackend(Path(tmpdir))
        item = WorkItem(id="", title="Test", description="")
        created = backend.create_item(item)

        updated = backend.update_item(created.id, status="approved")
        assert updated is not None
        assert updated.status == "approved"

        # Verify persisted
        loaded = backend.get_item(created.id)
        assert loaded is not None
        assert loaded.status == "approved"


def test_file_backend_delete():
    with tempfile.TemporaryDirectory() as tmpdir:
        backend = FileBackend(Path(tmpdir))
        item = WorkItem(id="", title="Test", description="")
        created = backend.create_item(item)

        assert backend.delete_item(created.id)
        assert backend.get_item(created.id) is None


def test_file_backend_filter_by_status():
    with tempfile.TemporaryDirectory() as tmpdir:
        backend = FileBackend(Path(tmpdir))

        backend.create_item(WorkItem(id="", title="A", description="", status="proposed"))
        backend.create_item(WorkItem(id="", title="B", description="", status="approved"))
        backend.create_item(WorkItem(id="", title="C", description="", status="proposed"))

        proposed = backend.list_items(status="proposed")
        assert len(proposed) == 2

        approved = backend.list_items(status="approved")
        assert len(approved) == 1
        assert approved[0].title == "B"
