import { execFileSync } from "node:child_process";
import { randomUUID } from "node:crypto";

// Launch-local extension: no edits to Pi's resources, settings or trust records.
export default function (pi) {
  let turn;
  let started = false;
  let events = [];
  let refreshMoment = "start";
  let context = "";
  let lastProgress = 0;
  let pendingInput = 0;
  let steering;
  const tools = new Set();
  const invoke = (command, payload, args = []) => execFileSync(process.env.LF_BIN,
    [command, ...args], { input: JSON.stringify(payload), encoding: "utf8",
      timeout: 30000, maxBuffer: 32 * 1024 * 1024, stdio: ["pipe", "pipe", "pipe"] });
  const readContext = (ctx, moment, hook) => JSON.parse(invoke("__context-block", {
    session_id: ctx.sessionManager.getSessionFile(), hook_event_name: hook,
    adapter_invocation_id: randomUUID(),
  }, ["--moment", moment])).hookSpecificOutput.additionalContext;
  const shutdown = (ctx) => {
    clearInterval(steering);
    ctx.abort(); process.exitCode = 1; ctx.shutdown();
  };
  const report = (event, ctx, extra = {}) => {
    if (!spec.native || (!started && event.type !== "session_start")) return;
    try {
      invoke("__pi-event", { ...event, ...extra, sessionFile: ctx.sessionManager.getSessionFile(),
        turn, openTools: tools.size, pendingInput });
    } catch (error) {
      shutdown(ctx);
      throw error;
    }
  };
  pi.on("session_start", async (event, ctx) => {
    report({ type: "session_start" }, ctx);
    started = true;
  });
  pi.on("session_compact", async () => { refreshMoment = "compact"; });
  pi.on("before_agent_start", async () => { refreshMoment ??= "start"; });
  // Automatic compaction continues without before_agent_start. Project context
  // into each request, without persisting it in Pi's conversation or tool state.
  pi.on("context_with_system", async (event, ctx) => {
    if (!process.env.LF_CONTEXT_DELIVERY) return;
    try {
      if (refreshMoment) {
        context = readContext(ctx, refreshMoment, "PiContext");
        refreshMoment = undefined;
      }
      const [head, ...rest] = event.messages;
      if (head?.role !== "system") throw new Error("Pi request has no leading system message");
      return { messages: [{ ...head, sections: { ...head.sections, loopflow: context } }, ...rest] };
    } catch (error) {
      shutdown(ctx);
      throw error;
    }
  });
  pi.on("agent_start", async (event, ctx) => {
    // Retries can start multiple low-level runs before agent_settled.
    if (turn) return;
    turn = randomUUID(); events = []; tools.clear();
    report({ type: "agent_start" }, ctx);
    if (process.env.LF_CONTEXT_DELIVERY) {
      steering = setInterval(() => {
        try {
          const content = readContext(ctx, "tool", "PiSteer");
          if (content) {
            // With triggerTurn omitted, Pi queues during streaming and only
            // stores if idle. A settlement race must never start another run.
            pi.sendMessage({ customType: "loopflow-steer", content, display: true }, { deliverAs: "steer" });
          }
        } catch (error) {
          ctx.ui.notify(`Loopflow steering failed: ${error.message}`, "error");
          shutdown(ctx);
        }
      }, 1000);
      steering.unref();
    }
  });
  pi.on("message_end", async (event, ctx) => {
    if (spec.native && event.message.role === "assistant") events.push(event);
    report(event, ctx, { events });
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
  pi.on("session_shutdown", async () => { clearInterval(steering); });
  pi.on("agent_settled", async (event, ctx) => {
    clearInterval(steering);
    report(event, ctx, { events });
    turn = undefined; events = []; tools.clear();
    if (process.env.LF_CONTEXT_DELIVERY) {
      try { invoke("__planning-check", {}); } catch { /* Next boundary retries planning. */ }
    }
  });
}
