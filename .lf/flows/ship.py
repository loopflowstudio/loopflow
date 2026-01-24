from loopflow.lf.flows import Choose, Flow

SHIP = Flow(
    Choose(
        options={
            "add_to_roadmap": Flow(
                {
                    "fork": [
                        {
                            "step": "roadmap",
                            "config": {"model": "claude", "goal": ["product-engineer"]},
                        },
                        {
                            "step": "roadmap",
                            "config": {"model": "gemini", "goal": ["ceo"]},
                        },
                        {
                            "step": "roadmap",
                            "config": {"model": "codex", "goal": ["designer"]},
                        },
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
