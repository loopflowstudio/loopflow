import { createBashToolDefinition, createLocalBashOperations } from "@earendil-works/pi-coding-agent";
import { execFileSync, spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { randomUUID } from "node:crypto";
import { appendFileSync, closeSync, constants, mkdirSync, mkdtempSync, openSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

// Keep Pi's shell backend, rendering and settings. Only the built-in bash tool
// is wrapped; extension-provided tools retain their own execution ownership.
async function guardBashTools(pi, ctx) {
  const settings = pi.getSettings();
  const local = createLocalBashOperations({ shellPath: settings.shellPath });
  const quote = (value) => `'${value.replaceAll("'", "'\\''")}'`;
  const operations = {
    async exec(command, cwd, options) {
      const directory = mkdtempSync(join(tmpdir(), "loopflow-pi-tool-"));
      const path = join(directory, "lifeline");
      let writer;
      try {
        execFileSync("/usr/bin/mkfifo", [path]);
        // RDWR opens without waiting for the shell's reader. OS exit also
        // closes this writer, even when Pi cannot run any shutdown handlers.
        writer = openSync(path, constants.O_RDWR);
        // The guard records the tool boundary; a store it cannot reach is a
        // logged gap, never a failed tool. The command runs either way.
        const guard = `${quote(spec.executable)} __context-block --moment tool-group --lifeline ${quote(path)} --provider-pid ${process.pid}`;
        return await local.exec(`${guard}\n${command}`, cwd, options);
      } finally {
        if (writer !== undefined) closeSync(writer);
        rmSync(directory, { recursive: true, force: true });
      }
    },
  };
  if (pi.getAllTools().some((tool) => tool.name === "bash" && tool.sourceInfo.source === "builtin")) {
    const active = pi.getActiveTools();
    pi.registerTool(createBashToolDefinition(ctx.cwd, {
      operations, shellPath: settings.shellPath, commandPrefix: settings.shellCommandPrefix,
    }));
    pi.setActiveTools(active);
  }
  // Pi gives an earlier extension's user_bash handler first refusal.
  pi.on("user_bash", async () => ({ operations }));
}

// Launch-local extension: no edits to Pi's resources, settings or trust records.
// Every launch gets the bash guard and the context block. A bare headless run
// (`spec.managed` false) polls for no steers and reports no turn end.
export default function (pi) {
  let turn;
  let started = false;
  let events = [];
  let refreshMoment = "start";
  let context = "";
  let lastProgress = 0;
  let pendingInput = 0;
  let steering;
  let updates;
  const stopUpdates = () => { updates?.kill(); updates = undefined; };
  const watchUpdates = (ctx) => {
    stopUpdates();
    if (!spec.native || !process.env.LF_CONTEXT_DELIVERY) return;
    const session = ctx.sessionManager.getSessionFile();
    const child = spawn(spec.executable, ["__context-block", "--moment", "task-updates"],
      { stdio: ["pipe", "pipe", "ignore"] });
    updates = child;
    child.stdin.on("error", () => {});
    child.on("error", (error) => recordingGap("Task updates", error));
    child.stdin.end(JSON.stringify({ session_id: session, hook_event_name: "PiTaskUpdates" }));
    createInterface({ input: child.stdout }).on("line", (line) => {
      // A queued line from the previous selection must not reach a new Session.
      if (updates !== child || ctx.sessionManager.getSessionFile() !== session) return;
      try {
        const update = JSON.parse(line);
        if (update.session !== session) return;
        pi.sendMessage({ customType: "loopflow-task-update", content: update.text, display: false },
          { triggerTurn: true, deliverAs: "followUp" });
      } catch (error) { recordingGap("Task update", error); }
    });
  };
  const tools = new Set();
  const invoke = (command, payload, args = []) => execFileSync(spec.executable,
    [command, ...args], { input: JSON.stringify(payload), encoding: "utf8",
      timeout: 30000, maxBuffer: 32 * 1024 * 1024, stdio: ["pipe", "pipe", "pipe"] });
  const readContext = (ctx, moment, hook) => JSON.parse(invoke("__context-block", {
    session_id: ctx.sessionManager.getSessionFile(), hook_event_name: hook,
    adapter_invocation_id: randomUUID(),
  }, ["--moment", moment])).hookSpecificOutput.additionalContext;
  const shutdown = (ctx) => {
    clearInterval(steering);
    stopUpdates();
    ctx.abort(); process.exitCode = 1; ctx.shutdown();
  };
  // lf's records are best-effort: an lf the extension cannot run is a recording
  // gap, never a failed turn or a notice in the conversation. Like Claude's
  // hooks, it shows nothing: one line per changed failure in the extension log
  // (lf's own process records the gap in logs/lf.log when it runs), then quiet
  // until the next success closes it.
  let gap;
  const log = (line) => {
    try {
      mkdirSync(dirname(spec.log), { recursive: true });
      appendFileSync(spec.log, `${new Date().toISOString()} ${line}\n`);
    } catch { /* A log lf cannot keep is no reason to stop the conversation. */ }
  };
  const recordingGap = (what, error) => {
    const message = `${what}: ${error.message.split("\n")[0]}`;
    if (gap === message) return;
    gap = message;
    log(`recording gap: ${message}; the conversation goes on without lf's records`);
  };
  const recovered = () => {
    if (gap === undefined) return;
    log(`recording gap closed (${gap}); lf records from here`);
    gap = undefined;
  };
  const report = (event, ctx, extra = {}) => {
    if (!spec.native || (!started && event.type !== "session_start")) return;
    try {
      invoke("__context-block", { ...event, ...extra, sessionFile: ctx.sessionManager.getSessionFile(),
        turn, openTools: tools.size, pendingInput }, ["--moment", "pi-event"]);
      recovered();
    } catch (error) {
      recordingGap("turn record", error);
    }
  };
  pi.on("session_start", async (event, ctx) => {
    try { await guardBashTools(pi, ctx); } catch (error) {
      shutdown(ctx);
      throw error;
    }
    report({ type: "session_start" }, ctx);
    started = true;
    watchUpdates(ctx);
  });
  pi.on("session_compact", async () => { refreshMoment = "compact"; });
  pi.on("before_agent_start", async () => { refreshMoment ??= "start"; });
  // Automatic compaction continues without before_agent_start. Project context
  // into each request, without persisting it in Pi's conversation or tool state.
  pi.on("context_with_system", async (event, ctx) => {
    if (!process.env.LF_CONTEXT_DELIVERY) return;
    if (refreshMoment) {
      // A refresh lf cannot serve keeps the last context and tries again next request.
      try {
        context = readContext(ctx, refreshMoment, "PiContext");
        refreshMoment = undefined;
        recovered();
      } catch (error) {
        recordingGap("context refresh", error);
      }
    }
    try {
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
    if (spec.managed && process.env.LF_CONTEXT_DELIVERY) {
      steering = setInterval(() => {
        try {
          const content = readContext(ctx, "tool", "PiSteer");
          recovered();
          if (content) {
            // With triggerTurn omitted, Pi queues during streaming and only
            // stores if idle. A settlement race must never start another run.
            pi.sendMessage({ customType: "loopflow-steer", content, display: true }, { deliverAs: "steer" });
          }
        } catch (error) {
          recordingGap("steering", error);
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
  pi.on("session_shutdown", async () => { clearInterval(steering); stopUpdates(); });
  pi.on("agent_settled", async (event, ctx) => {
    clearInterval(steering);
    report(event, ctx, { events });
    turn = undefined; events = []; tools.clear();
    if (spec.managed && process.env.LF_CONTEXT_DELIVERY) {
      try { invoke("__context-block", {}, ["--moment", "stop"]); } catch { /* Next boundary retries planning. */ }
    }
  });
}
