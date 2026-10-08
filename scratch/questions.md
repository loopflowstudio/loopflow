# LOO-440 assumptions (2026-10-08)

- Jack Heart's invocation-wide override takes precedence even over an explicit
  agent on a nested authored command. An independent command starts a new scope.
- Native history cannot transfer between providers. A conflicting resume reports
  the saved provider and the new-Session command before launching; a model override
  within that provider is allowed. No history is converted or discarded.
- The fixture uses synthetic account records and `--isolate`. Its earlier shared
  activation failed at a macOS Keychain write; no real provider or login was run.
- `lf context --skill realign` reports the generated local Work seed at 17,255
  tokens, 1,255 above its goal limit. It includes the active implementation diff;
  authored memory and scratch are curated separately. The Task directive and
  required regression coverage remain intact; no budget was raised.
