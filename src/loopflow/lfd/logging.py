"""Logging configuration for lfd daemon and agents."""

import logging
import sys
from pathlib import Path

LFD_LOG_DIR = Path.home() / ".lf" / "logs"
LFD_LOG_FILE = LFD_LOG_DIR / "lfd.log"


def get_lfd_logger(name: str = "lfd") -> logging.Logger:
    """Get or create the lfd logger.

    Logs to both stderr and ~/.lf/logs/lfd.log
    """
    logger = logging.getLogger(name)

    if logger.handlers:
        return logger

    logger.setLevel(logging.DEBUG)

    # Ensure log directory exists
    LFD_LOG_DIR.mkdir(parents=True, exist_ok=True)

    # File handler - detailed logs
    file_handler = logging.FileHandler(LFD_LOG_FILE)
    file_handler.setLevel(logging.DEBUG)
    file_format = logging.Formatter(
        "%(asctime)s | %(name)s | %(levelname)s | %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )
    file_handler.setFormatter(file_format)
    logger.addHandler(file_handler)

    # Stderr handler - warnings and above only
    stderr_handler = logging.StreamHandler(sys.stderr)
    stderr_handler.setLevel(logging.WARNING)
    stderr_format = logging.Formatter("%(levelname)s: %(message)s")
    stderr_handler.setFormatter(stderr_format)
    logger.addHandler(stderr_handler)

    return logger


# Module-level loggers for different components
agent_log = get_lfd_logger("lfd.agent")
worker_log = get_lfd_logger("lfd.worker")
stimulus_log = get_lfd_logger("lfd.stimulus")
