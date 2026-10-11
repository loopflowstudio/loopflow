# LOO-440 assumptions (2026-10-08)

- Jack Heart's invocation-wide override takes precedence even over an explicit
  agent on a nested authored command. An independent command starts a new scope.
- Native history cannot transfer between providers. A conflicting resume reports
  the saved provider and the new-Session command before launching; a model override
  within that provider is allowed. No history is converted or discarded.
- The fixture uses synthetic account records and `--isolate`. Its earlier shared
  activation failed at a macOS Keychain write; no real provider or login was run.
- The generated Task input embeds the full branch diff, including deletions.
  October 10 `lf context --skill gate --json`: goal 18,177/16,000 tokens;
  memory 15,993/16,000 and scratch 992/12,000 fit. The complete patch was
  inspected. Shrinking in-budget notes cannot remove this generated-input
  conflict without deleting implementation/evidence; limits remain unchanged.
