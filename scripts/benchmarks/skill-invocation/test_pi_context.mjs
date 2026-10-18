import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const root = mkdtempSync(join(tmpdir(), "pi-context-"));
const saved = { LF_BIN: process.env.LF_BIN, LF_CONTEXT_DELIVERY: process.env.LF_CONTEXT_DELIVERY };
try {
  const sourceFile = join(root, "context");
  const callback = join(root, "callback");
  writeFileSync(callback, `#!/bin/sh\ncat '${sourceFile}'\n`, { mode: 0o700 });
  process.env.LF_BIN = callback;
  process.env.LF_CONTEXT_DELIVERY = "fixture";
  const source = readFileSync(new URL("../../../rust/loopflow/src/harness/context_pi.mjs", import.meta.url), "utf8");
  const path = join(root, "extension.mjs");
  writeFileSync(path, `const spec = { native: false };\n${source}`);
  const hooks = new Map();
  (await import(path)).default({ on: (name, handler) => hooks.set(name, handler) });
  const ctx = { sessionManager: { getSessionFile: () => join(root, "pi.jsonl") } };
  const request = {
    messages: [
      { role: "system", content: "NATIVE_BASE", sections: { tools: "TOOL_RULES" }, toolsAdded: [{ name: "bash" }] },
      { role: "user", content: "REQUEST" },
    ],
  };
  const before = structuredClone(request);
  const setContext = (text) => writeFileSync(sourceFile, JSON.stringify({ hookSpecificOutput: { additionalContext: text } }));
  const project = () => hooks.get("context_with_system")(request, ctx);
  setContext("ORIGINAL");
  await hooks.get("before_agent_start")({}, ctx);
  let result = await project();
  assert.equal(result.messages[0].sections.loopflow, "ORIGINAL");
  setContext("NEW_SCRATCH 雪 NEW_STEER");
  assert.equal((await project()).messages[0].sections.loopflow, "ORIGINAL");
  // Automatic compaction skips before_agent_start; manual compaction uses it.
  for (const automatic of [true, false]) {
    await hooks.get("session_compact")({}, ctx);
    if (!automatic) await hooks.get("before_agent_start")({}, ctx);
    result = await project();
    assert.equal(result.messages[0].sections.loopflow, "NEW_SCRATCH 雪 NEW_STEER");
    assert.equal(result.messages[0].content, "NATIVE_BASE");
    assert.equal(result.messages[0].sections.tools, "TOOL_RULES");
    assert.deepEqual(result.messages[0].toolsAdded, before.messages[0].toolsAdded);
    assert.deepEqual(result.messages.slice(1), before.messages.slice(1));
  }
  assert.deepEqual(request, before, "request projection does not rewrite Pi history");
  delete process.env.LF_CONTEXT_DELIVERY;
  assert.equal(await project(), undefined, "unbound child receives no context");
  console.log("Pi context: start, automatic/manual compaction, tool preservation and unbound child pass");
} finally {
  for (const [key, value] of Object.entries(saved)) {
    if (value === undefined) delete process.env[key]; else process.env[key] = value;
  }
  rmSync(root, { recursive: true, force: true });
}
