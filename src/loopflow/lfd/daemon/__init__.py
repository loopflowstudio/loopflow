"""Daemon infrastructure for lfd.

This package contains the background service components:
- server.py: Unix socket server for IPC
- manager.py: Coordinates workers, manages concurrency slots
- client.py: Client for communicating with the daemon
- launchd.py: macOS launchd integration
- process.py: Process utilities
- protocol.py: Socket protocol definitions
"""
