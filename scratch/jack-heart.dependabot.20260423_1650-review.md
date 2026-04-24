# Review

## Validation

```bash
git diff --check
uv run python - <<'PY'
from pathlib import Path
import yaml

files = [Path('.github/dependabot.yml'), Path('.github/workflows/dependabot-auto.yml')]
for path in files:
    yaml.load(path.read_text(), Loader=yaml.BaseLoader)

workflow = yaml.load(Path('.github/workflows/dependabot-auto.yml').read_text(), Loader=yaml.BaseLoader)
assert workflow['on']['workflow_run']['workflows'] == ['CI']
assert set(workflow['jobs']) == {'enable-auto-merge', 'close-on-red'}
PY
```
