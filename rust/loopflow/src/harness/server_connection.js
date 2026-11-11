import { createConnection as connectMachineServer } from "node:net";

// Each boundary is one machine.sock connection, as a hook makes one request:
// connect, the hello a hook sends, one session.boundary, close. The server's
// connection slot is back as soon as the answer arrives. Nothing here throws:
// an unanswered boundary is a recording gap, one `recording gap` line per
// changed failure through `log`, and the next boundary sends the open gap and
// the printed batches.
function machineServer(delivery, log) {
  const adapter = process.env.LF_PROVIDER_CONTEXT ? JSON.parse(process.env.LF_PROVIDER_CONTEXT) : null;
  let queue = Promise.resolve();
  // Batches handed to the provider that no answered request has named yet.
  const printed = [];
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
    // While the server doesn't answer, a boundary that can't carry a steer
    // isn't worth another two seconds: it goes into the open gap instead.
    if (skippable && gap) return undefined;
    const claimed = [...printed];
    try {
      const answer = await ask({ kind: "session.boundary", moment, delivery, adapter, callback,
        gap: gap ? { first: gap.first, last: gap.last } : null, printed: claimed });
      printed.splice(0, claimed.length);
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
    // Printed first: a batch the provider took is named by the next request;
    // one lost before that arrives again, never not at all.
    printed(batch) { printed.push(batch); },
  };
}
