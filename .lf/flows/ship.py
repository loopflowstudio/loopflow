
SHIP = Flow(
    ChooseFork(
        options={
            "add_to_roadmap": ["roadmap"],
            "scope_from_roadmap": ["design_from_roadmap"],
        }
    ),
    "implement",
    "polish",
)


def flow():
    return SHIP
