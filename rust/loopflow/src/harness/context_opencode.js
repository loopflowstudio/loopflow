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
    let owner;
    // Context lf cannot serve is a recording gap, never a notice in the
    // conversation: like Claude's hooks, one line per changed failure in this
    // plugin's log (OpenCode's stderr, kept beside the capture), then quiet
    // until the next block lf renders closes it.
    let gap;
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
    // A launch that reports no turns opens no connection and reports no turn end.
    const server = spec.turns ? machineServer(delivery, (line) => console.error(line)) : undefined;
    const lf = { env: { ...process.env, LF_HOME: spec.delivery.home }, cwd: spec.delivery.repo };
    // The block comes from checkout files, rendered without the launch's
    // capability; the Session's news is read apart from it.
    const { LF_PROVIDER_CONTEXT: _, LF_BARE_STEER: __, ...renderEnv } = lf.env;
    const render = (moment, callback) => JSON.parse(execFileSync(spec.executable,
      ["__context-block", "--delivery", delivery, "--moment", moment], {
        env: renderEnv, cwd: lf.cwd, input: JSON.stringify(callback), encoding: "utf8", timeout: 30000,
      })).hookSpecificOutput.additionalContext;
    await session.hook("prompt", (input) => { owner ??= input.sessionID; });
    await session.hook("compaction", (input) => { if (input.sessionID === owner) refresh = "compact"; });
    await session.hook("context", async (input) => {
      owner ??= input.sessionID;
      if (input.sessionID !== owner) return;
      if (spec.instructions) input.system.push({ type: "text", text: spec.instructions });
      const callback = { session_id: input.sessionID, hook_event_name: "OpenCodeMessagesTransform",
        adapter_invocation_id: randomUUID() };
      await server?.boundary(refresh ?? "tool", callback);
      if (refresh) {
        try {
          block = render(refresh, callback);
          refresh = undefined;
          recovered();
        } catch (error) {
          recordingGap(error);
        }
      }
      const news = ownNews(callback, (line) => console.error(line), ["--delivery", delivery], lf);
      const text = `${news}${block ?? ""}`;
      if (text) input.system.push({ type: "text", text });
    });
    if (!server) return async () => {};
    const controller = new AbortController();
    const watching = (async () => {
      for await (const item of event.subscribe({ signal: controller.signal })) {
        if (item.type !== "session.execution.succeeded" || item.data.sessionID !== owner) continue;
        // An unanswered turn end leaves planning pending until a later boundary.
        await server.boundary("stop", {});
      }
    })().catch((error) => { if (!controller.signal.aborted) throw error; });
    return async () => { controller.abort(); await watching; };
  },
};
