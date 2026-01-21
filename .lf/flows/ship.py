
SHIP = Flow(
    Choose(
        options={
            "add_to_roadmap": Flow(
                {
                    "fork": [
                        {"step": "roadmap", "config": {"model": "claude", "voice": ["artist", "customer"]}},
                        {"step": "roadmap", "config": {"model": "gemini", "voice": ["blunt", "executive"]}},
                        {"step": "roadmap", "config": {"model": "codex", "voice": ["architect", "thorough"]}},
                    ]
                },
                {"join": {}},
            ),
            "scope_from_roadmap": Flow("design_from_roadmap", "implement", "polish"),
        }
    ),
)


def flow():
    return SHIP
