from loopflow.context import gather_prompt_components
from loopflow.config import load_config
from loopflow.tokens import analyze_components
from pathlib import Path

config = load_config(Path('.'))
comps = gather_prompt_components(Path('.'), inline='test', include_summaries=True, config=config)
print('has summaries:', comps.summaries is not None)
tree = analyze_components(comps)
print('categories:', list(tree.root.children.keys()))
