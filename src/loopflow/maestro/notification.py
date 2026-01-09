"""Notification event data structures."""

from dataclasses import dataclass, asdict
from datetime import datetime
from enum import Enum
from pathlib import Path


class NotificationType(Enum):
    STARTED = "started"
    WAITING = "waiting"
    COMPLETED = "completed"
    ERROR = "error"


@dataclass
class NotificationEvent:
    type: NotificationType
    session_id: str
    message: str
    timestamp: datetime
    repo: Path

    def to_dict(self) -> dict:
        """Serialize to dict for JSON."""
        d = asdict(self)
        d["type"] = self.type.value
        d["timestamp"] = self.timestamp.isoformat()
        d["repo"] = str(self.repo)
        return d

    @classmethod
    def from_dict(cls, data: dict) -> "NotificationEvent":
        """Deserialize from dict."""
        data = data.copy()
        data["type"] = NotificationType(data["type"])
        data["timestamp"] = datetime.fromisoformat(data["timestamp"])
        data["repo"] = Path(data["repo"])
        return cls(**data)
