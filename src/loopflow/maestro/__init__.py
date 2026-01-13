"""Maestro: session tracking, agent loops, and UI helpers."""

from loopflow.maestro.session import Session, SessionStatus
from loopflow.maestro.agent import (
    AgentLoopSpec,
    AgentStatus,
    OuterLoopConfig,
    OuterLoopMode,
    RegisteredAgent,
)
from loopflow.maestro.agents import (
    get_agent,
    get_agent_by_name,
    list_agents,
    register_agent,
    remove_agent,
    start_agent,
    stop_agent,
)

__all__ = [
    # Session
    "Session",
    "SessionStatus",
    # Agent
    "AgentLoopSpec",
    "AgentStatus",
    "OuterLoopConfig",
    "OuterLoopMode",
    "RegisteredAgent",
    # Agent API
    "get_agent",
    "get_agent_by_name",
    "list_agents",
    "register_agent",
    "remove_agent",
    "start_agent",
    "stop_agent",
]
