# maestro — Python Agent Support

Agent execution and state management. Called by lfd when running agents.

## Relationship to lfd

- lfd owns the daemon process and socket server
- maestro provides AgentRunner, triggers, and collector logic
- lfd imports from maestro to run agents

## Relationship to Swift Maestro

The Swift app (Maestro/) is the UI. This Python module is backend support.
They share the lfd.db database but don't communicate directly.

## Session Model Note

This module has its own Session dataclass for backwards compatibility.
The canonical Session is in loopflow.lfd.models. Don't create new code
using loopflow.maestro.session.Session.
