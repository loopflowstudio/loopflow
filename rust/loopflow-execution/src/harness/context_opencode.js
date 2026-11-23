import { execFileSync } from "node:child_process";
import { randomUUID } from "node:crypto";

// Add launch instructions and fresh context to the model copy. Native guides,
// tools, titles, stored user messages and compaction remain provider-owned.
export default {
  id: "loopflow-context",
  async setup({ session, event }) {
    // The block is rendered at the first request and after each compaction,
    // then reused; a block lf could not render is tried again next request.
    let refresh = "start";
    let block;
    // A work session runs the conversation lf named; any other launch owns
    // the first one that prompts.
    let owner = spec.wake ?? undefined;
    // OpenCode keeps no system text between requests, so a steer heard once is
    // sent with every later request of this conversation, as a transcript would.
    let heard = "";
    // Context lf cannot serve is a recording gap, never a notice in the
    // conversation: like Claude's hooks, one line per changed failure in this
    // plugin's log (OpenCode's stderr, kept beside the capture), then quiet
    // until the next block lf renders closes it.
    let gap;
    const log = (line) => console.error(line);
    const recordingGap = (error) => {
      const message = error.message.split("\n")[0];
      if (gap === message) return;
      gap = message;
      console.error(`recording gap: context ${message}; the prompt goes on without lf's context`);
    };
    const recovered = () => {
      if (gap === undefined) return;
      console.error(`recording gap closed (context ${gap}); lf serves context from here`);
      gap = undefined;
    };
    // A work session takes the server's steers while idle, as a turn OpenCode
    // starts; one already running queues the message as a steer. The answer
    // shows in the TUI; the message is the model's, not something typed.
    // A reopened conversation keeps the model of its last turn, whatever the
    // configuration says: the launch's model is set before its first turn,
    // typed or woken, and a model picked in the TUI after that holds.
    let model = spec.model;
    const takeModel = async () => {
      if (!model) return;
      const at = model.indexOf("/");
      const ref = { providerID: model.slice(0, at), id: model.slice(at + 1) };
      model = undefined;
      try {
        await session.switchModel({ sessionID: owner, model: ref });
      } catch (error) {
        log(`model ${ref.providerID}/${ref.id} not set: ${String(error?.message ?? error).split("\n")[0]}`);
      }
    };
    const wake = spec.wake ? wakeSocket(async (thread, text) => {
      if (thread !== owner) throw new Error(`this OpenCode runs ${owner}, not ${thread}`);
      await takeModel();
      await session.synthetic({ sessionID: owner, text });
    }, log) : null;
    // A launch that reports no turns opens no connection and reports no turn end.
    const server = spec.turns ? machineServer(delivery, log, wake) : undefined;
    // A reopened work session runs no turn until someone types: it names its
    // socket now, and the server sends what was saved while it was closed.
    if (wake) {
      server.boundary("tool", { session_id: owner, hook_event_name: "OpenCodeMessagesTransform",
        adapter_invocation_id: randomUUID() });
    }
    // A work session's turn is recorded from its first request to its end, so
    // the server knows when it is idle.
    let turn = false;
    const lf = { env: { ...process.env, LF_HOME: spec.delivery.home }, cwd: spec.delivery.repo };
    // The block comes from checkout files, rendered without the launch's
    // capability; the Session's news is read apart from it.
    const { LF_PROVIDER_CONTEXT: _, LF_BARE_STEER: __, ...renderEnv } = lf.env;
    const render = (moment, callback) => JSON.parse(execFileSync(spec.executable,
      ["__context-block", "--delivery", delivery, "--moment", moment], {
        env: renderEnv, cwd: lf.cwd, input: JSON.stringify(callback), encoding: "utf8", timeout: 30000,
      })).hookSpecificOutput.additionalContext;
    await session.hook("prompt", async (input) => {
      owner ??= input.sessionID;
      if (input.sessionID === owner) await takeModel();
    });
    await session.hook("compaction", (input) => { if (input.sessionID === owner) refresh = "compact"; });
    await session.hook("context", async (input) => {
      owner ??= input.sessionID;
      if (input.sessionID !== owner) return;
      if (spec.instructions) input.system.push({ type: "text", text: spec.instructions });
      const callback = { session_id: input.sessionID, hook_event_name: "OpenCodeMessagesTransform",
        adapter_invocation_id: randomUUID() };
      await server?.boundary(refresh ?? "tool", callback);
      if (wake && !turn) {
        turn = true;
        await server.boundary("tool", { session_id: owner, hook_event_name: "UserPromptSubmit" },
          { skippable: true });
      }
      if (refresh) {
        try {
          block = render(refresh, callback);
          refresh = undefined;
          recovered();
        } catch (error) {
          recordingGap(error);
        }
      }
      heard += ownNews(callback, log, ["--delivery", delivery], lf);
      const text = `${heard}${block ?? ""}`;
      if (text) input.system.push({ type: "text", text });
    });
    if (!server) return async () => {};
    const controller = new AbortController();
    const watching = (async () => {
      for await (const item of event.subscribe({ signal: controller.signal })) {
        if (item.type !== "session.execution.succeeded" || item.data.sessionID !== owner) continue;
        // An unanswered turn end leaves planning pending until a later boundary.
        // A work session names its conversation, which records it as idle.
        turn = false;
        await server.boundary("stop", wake ? { session_id: owner, hook_event_name: "Stop" } : {});
      }
    })().catch((error) => { if (!controller.signal.aborted) throw error; });
    return async () => { controller.abort(); await watching; };
  },
};
