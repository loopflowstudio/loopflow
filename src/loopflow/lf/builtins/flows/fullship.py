from loopflow.lf.flows import Flow


def flow():
    """Full ship flow starting with interactive design."""
    return Flow("design", "implement", "reduce", "polish")
