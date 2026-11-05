import { execFileSync } from "node:child_process";
import { randomUUID } from "node:crypto";

// Add launch instructions and fresh context to the model copy. Native guides,
// tools, titles, stored user messages and compaction remain provider-owned.
export default {
  id: "loopflow-context",
  async setup({ session, event }) {
    let compacted = false;
    let owner;
    // Context lf cannot serve is a recording gap, never a notice in the
    // conversation: like Claude's hooks, one line per changed failure in this
    // plugin's log (OpenCode's stderr, kept beside the capture), then quiet
    // until the next prompt lf serves closes it. lf's own process records the
    // gap in logs/lf.log when it runs.
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
    await session.hook("prompt", (input) => { owner ??= input.sessionID; });
    await session.hook("compaction", (input) => { if (input.sessionID === owner) compacted = true; });
    await session.hook("context", (input) => {
      owner ??= input.sessionID;
      if (input.sessionID !== owner) return;
      if (spec.instructions) input.system.push({ type: "text", text: spec.instructions });
      try {
        const output = execFileSync(spec.executable, ["__context-block", "--delivery", delivery,
          "--moment", compacted ? "compact" : "start"], {
          env: { ...process.env, LF_HOME: spec.delivery.home },
          cwd: spec.delivery.repo,
          input: JSON.stringify({ session_id: input.sessionID,
            hook_event_name: "OpenCodeMessagesTransform", adapter_invocation_id: randomUUID() }),
          encoding: "utf8", timeout: 30000,
        });
        input.system.push({ type: "text", text: JSON.parse(output).hookSpecificOutput.additionalContext });
        recovered();
      } catch (error) {
        recordingGap(error);
      }
    });
    const controller = new AbortController();
    const watching = (async () => {
      for await (const item of event.subscribe({ signal: controller.signal })) {
        if (item.type !== "session.execution.succeeded" || item.data.sessionID !== owner) continue;
        try {
          execFileSync(spec.executable, ["__context-block", "--delivery", delivery,
            "--moment", "stop"], {
            env: { ...process.env, LF_HOME: spec.delivery.home }, cwd: spec.delivery.repo,
            timeout: 10000, stdio: "ignore",
          });
        } catch { /* Planning remains pending until a later boundary. */ }
      }
    })().catch((error) => { if (!controller.signal.aborted) throw error; });
    return async () => { controller.abort(); await watching; };
  },
};
