import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const root = mkdtempSync(join(tmpdir(), "loo444-plugin-"));
try {
  const contextFile = join(root, "source");
  writeFileSync(contextFile, "ORIGINAL");
  const callback = join(root, "callback");
  writeFileSync(callback, `#!/bin/sh
printf '{"hookSpecificOutput":{"additionalContext":"%s"}}' "$(cat '${contextFile}')"
`, { mode: 0o700 });
  const spec = {
    executable: callback,
    delivery: { home: root, repo: root },
    instructions: "FIXED_ADDITION",
  };
  const source = readFileSync(new URL(
    "../../../rust/loopflow/src/harness/context_opencode.js", import.meta.url,
  ), "utf8");
  const path = join(root, "plugin.mjs");
  writeFileSync(path, `const spec = ${JSON.stringify(spec)}; const delivery = 'fixture';\n${source}`);
  const plugin = (await import(path)).default;
  const hooks = new Map();
  const dispose = await plugin.setup({
    session: { hook: async (name, callback) => { hooks.set(name, callback); } },
    event: { subscribe: async function* ({ signal }) {
      await new Promise((resolve) => signal.addEventListener("abort", resolve, { once: true }));
    } },
  });
  const context = (sessionID = "main") => ({
    sessionID,
    system: [{ type: "text", text: "NATIVE_BASE\nAGENTS_GUIDE" }],
    messages: [{ role: "user", content: [{ type: "text", text: "REQUEST" }] }],
  });
  await hooks.get("prompt")({ sessionID: "main" });
  let input = context();
  const messages = structuredClone(input.messages);
  await hooks.get("context")(input);
  assert.deepEqual(input.system.map((part) => part.text), [
    "NATIVE_BASE\nAGENTS_GUIDE", "FIXED_ADDITION", "ORIGINAL",
  ]);
  assert.deepEqual(input.messages, messages, "stored user content stays untouched");
  writeFileSync(contextFile, "REFRESHED");
  await hooks.get("compaction")({ sessionID: "main" });
  input = context();
  await hooks.get("context")(input);
  assert.deepEqual(input.system.map((part) => part.text), [
    "NATIVE_BASE\nAGENTS_GUIDE", "FIXED_ADDITION", "REFRESHED",
  ]);
  assert.deepEqual(input.messages, messages);
  const sibling = context("sibling");
  const before = structuredClone(sibling);
  await hooks.get("prompt")({ sessionID: "sibling" });
  await hooks.get("context")(sibling);
  assert.deepEqual(sibling, before);
  await dispose();
  console.log("OpenCode plugin: refresh after compaction, native additions and sibling isolation pass");
} finally {
  rmSync(root, { recursive: true, force: true });
}
