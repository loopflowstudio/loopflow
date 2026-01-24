from loopflow.lf.flows import Flow, Fork


def flow():
    return Flow(
        Fork(
            {"goal": "infra-engineer"},
            {"goal": "designer"},
            {"goal": "product-engineer"},
            step="roadmap",
            synthesize={"goal": "ceo"},
        ),
    )
