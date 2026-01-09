"""Notification adapters for different platforms."""

import subprocess
from abc import ABC, abstractmethod

from loopflow.maestro.notification import NotificationEvent


class NotificationAdapter(ABC):
    """Base adapter for notification dispatching."""

    @abstractmethod
    def send(self, event: NotificationEvent) -> None:
        """Send notification for the event."""
        pass


class MacOSAdapter(NotificationAdapter):
    """macOS notification via osascript."""

    def send(self, event: NotificationEvent) -> None:
        title = f"lf {event.type.value}"
        subprocess.run(
            [
                "osascript",
                "-e",
                f'display notification "{event.message}" with title "{title}"',
            ],
            capture_output=True,
        )


class ClaudeCodeHooksAdapter(NotificationAdapter):
    """Integration with Claude Code hooks system (future)."""

    def send(self, event: NotificationEvent) -> None:
        # Future: register hooks that call back to maestro
        pass
