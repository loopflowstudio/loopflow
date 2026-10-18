import { execFileSync } from "node:child_process";
import { randomUUID } from "node:crypto";

// Launch-local extension: no edits to Pi's resources, settings or trust records.
export default function (pi) {
  let turn;
  let started = false;
  let events = [];
  let compacted = false;
  let lastProgress = 0;
  let pendingInput = 0;
  const tools = new Set();
  const invoke = (command, payload, args = []) => execFileSync(process.env.LF_BIN,
    [command, ...args], { input: JSON.stringify(payload), encoding: "utf8",
      timeout: 30000, maxBuffer: 32 * 1024 * 1024, stdio: ["pipe", "pipe", "pipe"] });
  const report = (event, ctx, extra = {}) => {
    if (!spec.native || (!started && event.type !== "session_start")) return;
    try {
      invoke("__pi-event", { ...event, ...extra, sessionFile: ctx.sessionManager.getSessionFile(),
        turn, openTools: tools.size, pendingInput });
    } catch (error) {
      ctx.abort(); process.exitCode = 1; ctx.shutdown();
      throw error;
    }
  };
  pi.on("session_start", async (event, ctx) => {
    report({ type: "session_start" }, ctx);
    started = true;
  });
  pi.on("session_compact", async () => { compacted = true; });
  pi.on("before_agent_start", async (event, ctx) => {
    if (!process.env.LF_CONTEXT_DELIVERY) return;
    try {
      const result = JSON.parse(invoke("__context-block", {
        session_id: ctx.sessionManager.getSessionFile(), hook_event_name: "PiBeforeAgentStart",
        adapter_invocation_id: randomUUID(),
      }, ["--moment", compacted ? "compact" : "start"]));
      compacted = false;
      return { systemPrompt: event.systemPrompt + "\n\n" + result.hookSpecificOutput.additionalContext };
    } catch (error) {
      ctx.abort(); process.exitCode = 1; ctx.shutdown();
      throw error;
    }
  });
  pi.on("agent_start", async (event, ctx) => {
    // Retries can start multiple low-level runs before agent_settled.
    if (turn) return;
    turn = randomUUID(); events = []; tools.clear();
    report({ type: "agent_start" }, ctx);
  });
  pi.on("message_end", async (event, ctx) => {
    if (event.message.role === "assistant") {
      if (spec.native) events.push(event);
    }
    report(event, ctx);
  });
  pi.on("message_update", async (_event, ctx) => {
    if (Date.now() - lastProgress < 5000) return;
    lastProgress = Date.now();
    report({ type: "working" }, ctx);
  });
  pi.on("tool_execution_start", async (event, ctx) => {
    tools.add(event.toolCallId);
    report(event, ctx);
  });
  pi.on("tool_execution_end", async (event, ctx) => {
    tools.delete(event.toolCallId);
    report(event, ctx);
  });
  pi.on("ui_prompt_start", async (event, ctx) => {
    pendingInput += 1;
    report(event, ctx);
  });
  pi.on("ui_prompt_end", async (event, ctx) => {
    pendingInput = Math.max(0, pendingInput - 1);
    report(event, ctx);
  });
  pi.on("agent_settled", async (event, ctx) => {
    report(event, ctx, { events });
    turn = undefined; events = []; tools.clear();
    if (process.env.LF_CONTEXT_DELIVERY) {
      try { invoke("__planning-check", {}); } catch { /* Next boundary retries planning. */ }
    }
  });
}
