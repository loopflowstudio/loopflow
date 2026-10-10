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
  const plugin = await (await import(path)).default();
  const user = () => ({
    info: { role: "user", id: "msg1", sessionID: "main" },
    parts: [{ type: "text", text: "REQUEST" }],
  });

  let output = { messages: [user()] };
  await plugin["chat.message"]({ sessionID: "main" });
  await plugin["experimental.chat.messages.transform"]({}, output);
  assert.equal(output.messages[0].parts[1].text, "ORIGINAL");
  writeFileSync(contextFile, "REFRESHED");
  output = { messages: [
    { info: { role: "assistant", summary: true, id: "summary1" }, parts: [] },
    user(),
  ] };
  await plugin["experimental.chat.messages.transform"]({}, output);
  assert.equal(output.messages[1].parts[1].text, "REFRESHED");

  let system = { system: ["NATIVE_BASE\nAGENTS_GUIDE\nFIXED_ADDITION"] };
  await plugin["experimental.chat.system.transform"]({ sessionID: "main" }, system);
  assert.equal(system.system.length, 1);
  system = { system: ["NATIVE_BASE\nAGENTS_GUIDE"] };
  await plugin["experimental.chat.system.transform"]({ sessionID: "main" }, system);
  assert.deepEqual(system.system, ["NATIVE_BASE\nAGENTS_GUIDE", "FIXED_ADDITION"]);

  const sibling = user();
  sibling.info.sessionID = "sibling";
  output = { messages: [sibling] };
  await plugin["experimental.chat.messages.transform"]({}, output);
  assert.equal(output.messages[0].parts.length, 1);
  console.log("OpenCode plugin: refresh, native additions and sibling isolation pass");
} finally {
  rmSync(root, { recursive: true, force: true });
}
