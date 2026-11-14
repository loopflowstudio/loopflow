import { createBashToolDefinition, createLocalBashOperations } from "@earendil-works/pi-coding-agent";
import { execFileSync } from "node:child_process";
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
        // The guard joins the tool's process group to the watchdog that ends
        // it with its provider. It records nothing; the command runs either way.
        const guard = `/bin/sh -c ${quote(spec.bashGuard)} lf-tool-lifeline ${quote(path)} ${process.pid}`;
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
// Every launch gets the bash guard and the context block, and during a turn
// reads its own steers. A launch that reports turns (`spec.turns`) also
// reports each event to the Machine server, one connection per boundary; one
// that doesn't opens none and reports no turn end.
export default function (pi) {
  let turn;
  let started = false;
  let events = [];
  let refreshMoment = "start";
  let context = "";
  let lastProgress = 0;
  let pendingInput = 0;
  const tools = new Set();
  // lf's records are best-effort: an lf the extension cannot run, or a server
  // it cannot reach, is a recording gap, never a failed turn or a notice in the
  // conversation. Like Claude's hooks, it shows nothing: one line per changed
  // failure in the extension log, then quiet until the next success closes it.
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
  const server = spec.turns && process.env.LF_CONTEXT_DELIVERY
    ? machineServer(process.env.LF_CONTEXT_DELIVERY, log) : undefined;
  // The block comes from checkout files, rendered without the launch's
  // capability; the Session's news is read apart from it.
  const { LF_PROVIDER_CONTEXT: _, LF_BARE_STEER: __, ...renderEnv } = process.env;
  const readContext = async (ctx, moment) => {
    const callback = { session_id: ctx.sessionManager.getSessionFile(), hook_event_name: "PiContext",
      adapter_invocation_id: randomUUID() };
    await server?.boundary(moment, callback);
    return JSON.parse(execFileSync(spec.executable, ["__context-block", "--moment", moment], {
      input: JSON.stringify(callback), encoding: "utf8", env: renderEnv,
      timeout: 30000, maxBuffer: 32 * 1024 * 1024, stdio: ["pipe", "pipe", "pipe"] }))
      .hookSpecificOutput.additionalContext;
  };
  const shutdown = (ctx) => {
    ctx.abort(); process.exitCode = 1; ctx.shutdown();
  };
  // Every event a native launch reports is recorded; a launch Pi's RPC driver
  // records reports nothing here. With the server not answering, only these
  // still wait for it: Pi's identity, a tool start and the turn end. Later
  // reports repeat the turn, open tools, pending input and assistant messages.
  const kept = new Set(["session_start", "tool_execution_start", "agent_settled"]);
  const report = async (event, ctx, extra = {}) => {
    if (!started && event.type !== "session_start") return;
    const sessionFile = ctx.sessionManager.getSessionFile();
    if (server && spec.native) {
      await server.boundary("event", { ...event, ...extra, sessionFile,
        turn, openTools: tools.size, pendingInput }, { skippable: !kept.has(event.type) });
    }
    // Pi takes a steer only while a turn runs; outside one it would be stored
    // without starting a run.
    if (!turn || event.type === "agent_settled") return;
    const content = ownNews({ session_id: sessionFile, hook_event_name: "PiSteer" }, log);
    if (!content) return;
    // With triggerTurn omitted, Pi queues during streaming and only stores if
    // idle. A settlement race must never start another run.
    pi.sendMessage({ customType: "loopflow-steer", content, display: true },
      { deliverAs: "steer" });
  };
  pi.on("session_start", async (event, ctx) => {
    try { await guardBashTools(pi, ctx); } catch (error) {
      shutdown(ctx);
      throw error;
    }
    await report({ type: "session_start" }, ctx);
    started = true;
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
        context = await readContext(ctx, refreshMoment);
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
    await report({ type: "agent_start" }, ctx);
  });
  pi.on("message_end", async (event, ctx) => {
    if (spec.native && event.message.role === "assistant") events.push(event);
    await report(event, ctx, { events });
  });
  // A steer saved while the model streams waits at most five seconds.
  pi.on("message_update", async (_event, ctx) => {
    if (Date.now() - lastProgress < 5000) return;
    lastProgress = Date.now();
    await report({ type: "working" }, ctx);
  });
  pi.on("tool_execution_start", async (event, ctx) => {
    tools.add(event.toolCallId);
    await report(event, ctx);
  });
  pi.on("tool_execution_end", async (event, ctx) => {
    tools.delete(event.toolCallId);
    await report(event, ctx);
  });
  pi.on("ui_prompt_start", async (event, ctx) => {
    pendingInput += 1;
    await report(event, ctx);
  });
  pi.on("ui_prompt_end", async (event, ctx) => {
    pendingInput = Math.max(0, pendingInput - 1);
    await report(event, ctx);
  });
  pi.on("agent_settled", async (event, ctx) => {
    await report(event, ctx, { events });
    turn = undefined; events = []; tools.clear();
    if (!server) return;
    // An unanswered turn end leaves planning pending until a later boundary.
    await server.boundary("stop", {});
  });
}
