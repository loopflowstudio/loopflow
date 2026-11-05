import { execFileSync } from "node:child_process";
import { randomUUID } from "node:crypto";

// Add launch instructions and fresh context to the model copy. Native guides,
// tools, titles, stored user messages and compaction remain provider-owned.
export default {
  id: "loopflow-context",
  async setup({ session, event }) {
    let compacted = false;
    let owner;
    await session.hook("prompt", (input) => { owner ??= input.sessionID; });
    await session.hook("compaction", (input) => { if (input.sessionID === owner) compacted = true; });
    await session.hook("context", (input) => {
      owner ??= input.sessionID;
      if (input.sessionID !== owner) return;
      if (spec.instructions) input.system.push({ type: "text", text: spec.instructions });
      // Context lf cannot serve (a store it cannot reach) is a gap lf logs
      // itself; the prompt goes on without it.
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
      } catch (error) {
        console.error(`loopflow context unavailable for this prompt: ${error.message.split("\n")[0]}`);
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
