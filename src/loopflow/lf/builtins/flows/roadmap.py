from loopflow.lf.flows import Flow, Fork, Synthesize


def flow():
    return Flow(
        Fork(
            {"step": "roadmap", "voice": "infra-engineer"},
            {"step": "roadmap", "voice": "designer"},
            {"step": "roadmap", "voice": "product-engineer"},
        ),
        Synthesize(goal="ceo"),
    )
