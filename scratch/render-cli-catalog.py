"""Render reviewed catalog rows without repeating caller citations per option."""
import json
from pathlib import Path


def _cell(value: str) -> str:
    return value.replace('|', '\\|').replace('\n', ' ')


def main() -> None:
    path = Path('scratch/cli-command-catalog.md')
    old = path.read_text()
    data = json.loads(Path('scratch/cli-catalog-verdicts.json').read_text())
    commands = {row['path']: row for row in data['commands']}
    baseline = "https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8"
    refs: dict[tuple[str, int], str] = {}
    notes: dict[str, str] = {}

    def evidence(row: dict) -> str:
        parts = []
        for name, line, kind in row['callers']:
            key = (name, line)
            refs.setdefault(key, f'E{len(refs):03}')
            parts.append(f'{kind}: [{refs[key]}]')
        return '; '.join(parts) or 'No literal caller found; usage unestablished'

    def note(text: str) -> str:
        notes.setdefault(text, f'N{len(notes):03}')
        return notes[text]

    out = [old.split('## Commands', 1)[0].rstrip(), '', '## Commands', '',
           'P:LINE refers to the [baseline Clap declarations](https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/mod.rs). E references are caller/source evidence. N references expand in the rationale section. Option caller references inherit the named command row; they do not claim every optional input is passed by that caller.', '',
           '| Row | Canonical path | Target owner | Purpose | Callers / source | Overlap | Verdict → target |',
           '|---|---|---|---|---|---|---|']
    for row in commands.values():
        overlap = row['overlap']
        if overlap.startswith('Owns '):
            overlap = 'Distinct operation described in purpose; no equivalent identified.'
        values = [row['row_id'], row['path'] + (' (hidden)' if row['hidden'] else ''),
                  row['owner'], row['description'], evidence(row) + f"; P:{row['source_line']}",
                  note(overlap), row['verdict'] + ' → ' + row['target']]
        out.append('| ' + ' | '.join(_cell(v) for v in values) + ' |')
    out += ['', '## Options and positional arguments', '',
            'Primary short and long flags share one row. Hidden and automatic arguments are included; required/default metadata remains in the raw JSON. Purpose and distinct effect justify retained options; literal caller absence alone is not evidence of a dead public input.', '',
            '| Row | Canonical path and argument | Owner / callers | Purpose | Overlap / source | Verdict → target |',
            '|---|---|---|---|---|---|']
    for row in data['arguments']:
        owner = commands[' '.join(row['path'])]
        label = row['canonical']
        if row['short'] and row['long']:
            label += f" / -{row['short']}"
        if row['hidden']:
            label += ' (hidden)'
        values = [row['row_id'], label, owner['row_id'], row['description'],
                  note(row['overlap']) + f"; P:{row['source_line']}", row['verdict'] + ' → ' + row['target']]
        out.append('| ' + ' | '.join(_cell(v) for v in values) + ' |')
    out += ['', '## Rationale key', '']
    out.extend(f'- **{key}**: {text}' for text, key in notes.items())
    out += ['', '## Caller evidence', '']
    out.extend(f'[{key}]: {baseline}/{name}#L{line}' for (name, line), key in refs.items())
    out += ['', '## Extra aliases' + old.split('## Extra aliases', 1)[1]]
    path.write_text('\n'.join(out))


if __name__ == '__main__':
    main()
