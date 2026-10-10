import { execFileSync } from "node:child_process";

// OpenCode invokes this plugin on terminal and server paths. Keep native prompt,
// guide loading, titles and compaction. Only additions go in the system channel.
export default async () => {
  let session;
  let current;
  const refresh = (moment) => {
    const output = execFileSync(spec.executable, ["__context-block", "--delivery", delivery, "--moment", moment], {
      env: { ...process.env, LF_HOME: spec.delivery.home },
      cwd: spec.delivery.repo,
      encoding: "utf8",
      timeout: 30000,
    });
    return JSON.parse(output).hookSpecificOutput.additionalContext;
  };
  return {
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
      const revision = summary?.info.id ?? "start";
      if (!current || current.revision !== revision) {
        current = { revision, text: refresh(summary ? "compact" : "start") };
      }
      // Transform the conversation copy, not the stored user request or title.
      user.parts.push({ type: "text", text: current.text, synthetic: true,
        id: `lf-context-${user.info.id}`, sessionID: session, messageID: user.info.id });
    },
  };
};
