import { execFileSync } from "node:child_process";
import { randomUUID } from "node:crypto";

// OpenCode invokes this plugin on terminal and server paths. Keep native prompt,
// guide loading, titles and compaction. Only additions go in the system channel.
export default async () => {
  let session;
  const refresh = (moment) => {
    const output = execFileSync(spec.executable, ["__context-block", "--delivery", delivery, "--moment", moment], {
      env: { ...process.env, LF_HOME: spec.delivery.home },
      cwd: spec.delivery.repo,
      input: JSON.stringify({ session_id: session, hook_event_name: "OpenCodeMessagesTransform",
        adapter_invocation_id: randomUUID() }),
      encoding: "utf8",
      timeout: 30000,
    });
    return JSON.parse(output).hookSpecificOutput.additionalContext;
  };
  return {
    event: async ({ event }) => {
      if (event.type !== "session.idle" || event.properties.sessionID !== session) return;
      try {
        execFileSync(spec.executable, ["__planning-check", "--delivery", delivery], {
          env: { ...process.env, LF_HOME: spec.delivery.home }, cwd: spec.delivery.repo,
          timeout: 10000, stdio: "ignore",
        });
      } catch { /* Planning remains local and pending until another boundary. */ }
    },
    "chat.message": async (input) => {
      session ??= input.sessionID;
    },
    "experimental.chat.system.transform": async (input, output) => {
      if (input.sessionID !== session) return;
      if (spec.instructions && !output.system.some((text) => text.includes(spec.instructions))) output.system.push(spec.instructions);
    },
    "experimental.chat.messages.transform": async (_, output) => {
      const user = output.messages.findLast((message) => message.info.role === "user");
      if (!user) return;
      session ??= user.info.sessionID;
      if (user.info.sessionID !== session) return;
      const summary = output.messages.findLast((message) => message.info.role === "assistant" && message.info.summary);
      // Each callback owns an adapter invocation, not a provider request. The
      // provider retries this transformed copy internally; later callbacks must
      // acquire fresh context instead of caching a consumed steer batch.
      const text = refresh(summary ? "compact" : "start");
      // Transform the conversation copy, not the stored user request or title.
      user.parts = user.parts.filter((part) => part.id !== `lf-context-${user.info.id}`);
      user.parts.push({ type: "text", text, synthetic: true,
        id: `lf-context-${user.info.id}`, sessionID: session, messageID: user.info.id });
    },
  };
};
