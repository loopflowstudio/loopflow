"""Headless Complete journey; these list consumers are not native UI views."""

import json
from pathlib import Path
import tempfile
import unittest


class Store:
    def __init__(self, path: Path) -> None:
        self.path = path

    def read(self) -> dict:
        return json.loads(self.path.read_text())

    def complete(self, task: str, allowed: bool) -> dict:
        if not allowed:
            raise PermissionError("completion refused")
        state = self.read()
        state["tasks"].remove(task)
        state["revision"] += 1
        self.path.write_text(json.dumps(state))
        return state


class TaskList:
    def __init__(self, store: Store) -> None:
        self.store = store
        self.revision = -1
        self.tasks = []
        self.selected = None
        self.receive(store.read())

    def receive(self, state: dict) -> None:
        if state["revision"] < self.revision:
            return
        self.revision = state["revision"]
        self.tasks = list(state["tasks"])
        if self.selected not in self.tasks:
            self.selected = next(iter(self.tasks), None)

    def complete(self, task: str, consumers: list["TaskList"], allowed: bool = True) -> None:
        if not allowed:
            raise PermissionError("completion refused")
        self.tasks.remove(task)
        self.selected = next(iter(self.tasks), None)


class CompleteJourney(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        path = Path(self.directory.name) / "tasks.json"
        path.write_text(json.dumps({"revision": 0, "tasks": ["one", "two"]}))
        self.store = Store(path)
        self.desktop = TaskList(self.store)
        self.mobile = TaskList(self.store)
        self.consumers = [self.desktop, self.mobile]

    def test_complete_reaches_both_lists_and_survives_stale_reply_and_restart(self) -> None:
        stale = self.store.read()
        self.desktop.complete("one", self.consumers)
        for consumer in self.consumers:
            self.assertEqual(consumer.tasks, ["two"])
            self.assertEqual(consumer.selected, "two")
            consumer.receive(stale)
            self.assertEqual(consumer.tasks, ["two"])
            self.assertEqual(consumer.selected, "two")
        restarted = TaskList(Store(self.store.path))
        self.assertEqual(restarted.tasks, ["two"])
        self.assertEqual(restarted.selected, "two")

    def test_refusal_keeps_saved_task_and_both_selections(self) -> None:
        with self.assertRaises(PermissionError):
            self.mobile.complete("one", self.consumers, allowed=False)
        self.assertEqual(self.store.read()["tasks"], ["one", "two"])
        for consumer in self.consumers:
            self.assertEqual(consumer.tasks, ["one", "two"])
            self.assertEqual(consumer.selected, "one")

    def test_completing_last_task_clears_selection_on_both_lists(self) -> None:
        self.desktop.complete("one", self.consumers)
        self.mobile.complete("two", self.consumers)
        for consumer in self.consumers:
            self.assertEqual(consumer.tasks, [])
            self.assertIsNone(consumer.selected)


if __name__ == "__main__":
    unittest.main()
