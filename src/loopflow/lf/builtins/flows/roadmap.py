from loopflow.lf.flows import Flow, Fork


def flow():
    return Flow(
        Fork(
            {"direction": "infra-engineer"},
            {"direction": "designer"},
            {"direction": "product-engineer"},
            step="roadmap",
            synthesize={"direction": "ceo"},
        ),
    )
