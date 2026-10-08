# LOO-440 assumptions (2026-10-08)

- Jack Heart's invocation-wide override takes precedence even over an explicit
  agent on a nested authored command. An independent command starts a new scope.
- Native history cannot transfer between providers. A conflicting resume reports
  the saved provider and the new-Session command before launching; a model override
  within that provider is allowed. No history is converted or discarded.
- The fixture uses synthetic account records and `--isolate`. Its earlier shared
  activation failed at a macOS Keychain write; no real provider or login was run.
- The generated Task input embeds the complete branch diff, including deletions.
  Reconciliation's `lf context --skill realign --json` reports about 17,900
  tokens against the 16,000-token goal limit (about 1,900 over). Memory and
  scratch fit. Required implementation and regression evidence account for the
  remaining conflict; shrinking authored notes cannot remove it. The full
  excerpt source was read, evidence retained and limits left unchanged.
