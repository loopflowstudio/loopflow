import { statSync as markerStamp } from "node:fs";
import { createConnection as connectMachineServer } from "node:net";

// Every launch reads its own news: its unread steers and what changed on its
// Tasks. It compares the markers a steer for it touches against its last
// check, as a Claude tool hook does, and only when one is newer starts
// `lf __context-block --moment steer`, which claims them from the store.
// Nothing here throws: lf logs a store it cannot read, and the conversation
// goes on.
function ownNews(callback, log, args = [], options = {}) {
  const seen = process.env.LF_STEER_SEEN;
  if (!seen) return "";
  const stamp = (path) => {
    try { return path ? markerStamp(path).mtimeMs : undefined; } catch { return undefined; }
  };
  const checked = stamp(seen);
  const quiet = [spec.steers, process.env.LF_STEER_SESSION, process.env.LF_STEER_TASK]
    .every((marker) => { const saved = stamp(marker); return saved === undefined || checked > saved; });
  if (checked !== undefined && quiet) return "";
  try {
    const output = execFileSync(spec.executable, ["__context-block", ...args, "--moment", "steer"], {
      env: process.env, ...options, input: JSON.stringify(callback), encoding: "utf8", timeout: 10000,
      stdio: ["pipe", "pipe", "pipe"] });
    return output.trim() ? JSON.parse(output).hookSpecificOutput.additionalContext : "";
  } catch (error) {
    log(`steer check failed: ${error.message.split("\n")[0]}`);
    return "";
  }
}

// A held launch reports each boundary for the server to record: one
// machine.sock connection, as a hook makes one request: connect, the hello a
// hook sends, one session.boundary, close. The server's connection slot is
// back as soon as the answer arrives. Nothing here throws: an unanswered
// boundary is a recording gap, one `recording gap` line per changed failure
// through `log`, and the next boundary sends the open gap.
function machineServer(delivery, log) {
  const adapter = process.env.LF_PROVIDER_CONTEXT ? JSON.parse(process.env.LF_PROVIDER_CONTEXT) : null;
  let queue = Promise.resolve();
  let gap;

  // The two seconds cover connecting, the hello and the answer.
  const ask = (operation) => new Promise((resolve, reject) => {
    if (!spec.server) throw new Error("Machine server socket unavailable");
    const socket = connectMachineServer(spec.server.socket);
    // The provider's exit never waits on this connection.
    socket.unref();
    socket.setEncoding("utf8");
    let buffer = "";
    let timer;
    const finish = (error, result) => {
      clearTimeout(timer);
      socket.destroy();
      if (error) reject(error); else resolve(result);
    };
    timer = setTimeout(() => finish(new Error("Loopflow server did not answer within 2s")), 2000);
    timer.unref();
    socket.on("error", finish);
    socket.on("close", () => finish(new Error("Loopflow server closed the connection")));
    socket.on("data", (chunk) => {
      buffer += chunk;
      for (let end = buffer.indexOf("\n"); end >= 0; end = buffer.indexOf("\n")) {
        let message;
        try { message = JSON.parse(buffer.slice(0, end)); } catch (error) { return finish(error); }
        buffer = buffer.slice(end + 1);
        if (message.type === "welcome") {
          socket.write(`${JSON.stringify({ type: "request", id: "boundary", operation, select: null })}\n`);
        } else if (message.type === "response") {
          finish(undefined, message.result);
        } else if (message.type === "error") {
          finish(new Error(`${message.code}: ${message.message}`));
        }
      }
    });
    socket.write(`${JSON.stringify({ type: "hello", revision: spec.server.revision,
      min_revision: spec.server.min_revision, client: { kind: "plugin", platform: spec.server.platform,
        name: spec.server.name, version: spec.server.version } })}\n`);
  });
  const send = async (moment, callback, skippable) => {
    // Only a turn end needs no provider context.
    if (!adapter && moment !== "stop") return undefined;
    // While the server doesn't answer, a boundary later reports repeat isn't
    // worth another two seconds: it goes into the open gap instead.
    if (skippable && gap) return undefined;
    try {
      const answer = await ask({ kind: "session.boundary", moment, delivery, adapter, callback,
        gap: gap ? { first: gap.first, last: gap.last } : null, printed: [], inbox: null });
      if (gap) log(`recording gap closed (${gap.message}); lf records from here`);
      gap = undefined;
      return answer;
    } catch (error) {
      const now = Math.floor(Date.now() / 1000);
      const message = `session boundary: ${error.message.split("\n")[0]}`;
      if (gap?.message !== message) {
        log(`recording gap: ${message}; the conversation goes on without lf's records`);
      }
      gap = { first: gap?.first ?? now, last: now, message };
      return undefined;
    }
  };
  return {
    // Boundaries go to the server in the order the provider reported them.
    // A `skippable` one is dropped while a gap is open.
    boundary(moment, callback, { skippable = false } = {}) {
      const answer = queue.then(() => send(moment, callback, skippable));
      queue = answer;
      return answer;
    },
  };
}
