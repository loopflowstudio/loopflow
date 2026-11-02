import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const root = mkdtempSync(join(tmpdir(), "pi-context-"));
const saved = { LF_BIN: process.env.LF_BIN, LF_CONTEXT_DELIVERY: process.env.LF_CONTEXT_DELIVERY };
try {
  // SDK tool execution is covered by the real-Pi probes. This context-only
  // test runs without an installed provider and fails if it reaches tools.
  const sdk = join(root, "node_modules/@earendil-works/pi-coding-agent");
  mkdirSync(sdk, { recursive: true });
  writeFileSync(join(sdk, "package.json"), JSON.stringify({ type: "module", exports: "./index.js" }));
  writeFileSync(join(sdk, "index.js"), `
    export function createBashToolDefinition() { throw new Error("unexpected tool execution"); }
    export function createLocalBashOperations() { return {}; }
  `);
  const sourceFile = join(root, "context");
  const callback = join(root, "callback");
  writeFileSync(callback, `#!/bin/sh\ncat '${sourceFile}'\n`, { mode: 0o700 });
  process.env.LF_BIN = callback;
  process.env.LF_CONTEXT_DELIVERY = "fixture";
  const source = readFileSync(new URL("../../../rust/loopflow/src/harness/context_pi.mjs", import.meta.url), "utf8");
  const path = join(root, "extension.mjs");
  writeFileSync(path, `const spec = ${JSON.stringify({ native: false, executable: callback })};\n${source}`);
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
  // Real child pipe, simulated provider API: automatic reports leave the editor
  // and selected native Session untouched, and reject delayed foreign frames.
  const nativePath = join(root, "native-extension.mjs");
  const nativeSession = ctx.sessionManager.getSessionFile();
  writeFileSync(callback, `#!/bin/sh
if [ "$3" = "task-updates" ]; then
  cat >/dev/null
  printf '%s\\n' '${JSON.stringify({ session: nativeSession, text: "gate failed" })}' '${JSON.stringify({ session: "other-session", text: "wrong" })}'
else
  cat >/dev/null
fi
`, { mode: 0o700 });
  writeFileSync(nativePath, `const spec = ${JSON.stringify({ native: true, executable: callback })};\n${source}`);
  const nativeHooks = new Map();
  const messages = [];
  const draft = "unfinished draft 雪";
  const nativeCtx = { ...ctx, cwd: root, ui: { getEditorText: () => draft, notify: (message) => { throw new Error(message); } } };
  (await import(nativePath)).default({
    on: (name, handler) => nativeHooks.set(name, handler), getSettings: () => ({}),
    getAllTools: () => [], sendMessage: (message, options) => messages.push({ message, options }),
  });
  await nativeHooks.get("session_start")({}, nativeCtx);
  const deadline = Date.now() + 5000;
  while (!messages.length && Date.now() < deadline) await new Promise((resolve) => setTimeout(resolve, 10));
  await nativeHooks.get("session_shutdown")();
  assert.deepEqual(messages, [{ message: { customType: "loopflow-task-update", content: "gate failed", display: true }, options: { triggerTurn: true, deliverAs: "followUp" } }]);
  assert.equal(nativeCtx.ui.getEditorText(), draft);
  assert.equal(nativeCtx.sessionManager.getSessionFile(), nativeSession);
  delete process.env.LF_CONTEXT_DELIVERY;
  assert.equal(await project(), undefined, "unbound child receives no context");
  console.log("Pi context: start, automatic/manual compaction, tool preservation and unbound child pass");
} finally {
  for (const [key, value] of Object.entries(saved)) {
    if (value === undefined) delete process.env[key]; else process.env[key] = value;
  }
  rmSync(root, { recursive: true, force: true });
}
