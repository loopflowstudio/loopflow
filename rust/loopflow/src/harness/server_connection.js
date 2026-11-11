import { createConnection as connectMachineServer } from "node:net";

// One machine.sock connection for this conversation, opened with the hello a
// hook sends; each boundary is one session.boundary request on it. Nothing here
// throws: an unanswered boundary is a recording gap, one `recording gap` line
// per changed failure through `log`, and the next boundary connects again with
// the open gap and the printed batches. `close` after a turn end gives the
// server its connection slot back until the next turn.
function machineServer(delivery, log) {
  const adapter = process.env.LF_PROVIDER_CONTEXT ? JSON.parse(process.env.LF_PROVIDER_CONTEXT) : null;
  const answers = new Map();
  let socket;
  let welcomed;
  let next = 0;
  let queue = Promise.resolve();
  // Batches handed to the provider that no answered request has named yet.
  const printed = [];
  let gap;

  const drop = (error) => {
    socket?.destroy();
    socket = undefined;
    welcomed = undefined;
    for (const { reject } of answers.values()) reject(error);
    answers.clear();
  };
  const connect = () => new Promise((resolve, reject) => {
    if (!spec.server) throw new Error("Machine server socket unavailable");
    const opened = connectMachineServer(spec.server.socket);
    socket = opened;
    // The provider's exit never waits on this connection.
    opened.unref();
    opened.setEncoding("utf8");
    let buffer = "";
    let hello = { resolve, reject };
    const fail = (error) => {
      if (socket !== opened) return;
      hello?.reject(error);
      hello = undefined;
      drop(error);
    };
    opened.on("error", fail);
    opened.on("close", () => fail(new Error("Loopflow server closed the connection")));
    opened.on("data", (chunk) => {
      buffer += chunk;
      for (let end = buffer.indexOf("\n"); end >= 0; end = buffer.indexOf("\n")) {
        let message;
        try { message = JSON.parse(buffer.slice(0, end)); } catch (error) { return fail(error); }
        buffer = buffer.slice(end + 1);
        if (message.type === "welcome") {
          hello?.resolve();
          hello = undefined;
        } else if (message.type === "response") {
          answers.get(message.id)?.resolve(message.result);
          answers.delete(message.id);
        } else if (message.type === "error" && message.id && answers.has(message.id)) {
          answers.get(message.id).reject(new Error(`${message.code}: ${message.message}`));
          answers.delete(message.id);
        } else if (message.type === "error") {
          fail(new Error(`${message.code}: ${message.message}`));
        }
      }
    });
    opened.write(`${JSON.stringify({ type: "hello", revision: spec.server.revision,
      min_revision: spec.server.min_revision, client: { kind: "plugin", platform: spec.server.platform,
        name: spec.server.name, version: spec.server.version } })}\n`);
  });
  const ask = (operation) => new Promise((resolve, reject) => {
    const id = `b-${++next}`;
    answers.set(id, { resolve, reject });
    socket.write(`${JSON.stringify({ type: "request", id, operation, select: null })}\n`);
  });
  const within = (promise) => {
    let timer;
    const late = new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error("Loopflow server did not answer within 2s")), 2000);
    });
    return Promise.race([promise, late]).finally(() => clearTimeout(timer));
  };
  const send = async (moment, callback) => {
    // Only a turn end needs no provider context.
    if (!adapter && moment !== "stop") return undefined;
    const claimed = [...printed];
    const operation = { kind: "session.boundary", moment, delivery, adapter, callback,
      gap: gap ? { first: gap.first, last: gap.last } : null, printed: claimed };
    try {
      const answer = await within((async () => {
        await (welcomed ??= connect());
        return ask(operation);
      })());
      printed.splice(0, claimed.length);
      if (gap) log(`recording gap closed (${gap.message}); lf records from here`);
      gap = undefined;
      return answer;
    } catch (error) {
      drop(error);
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
    boundary(moment, callback) {
      const answer = queue.then(() => send(moment, callback));
      queue = answer;
      return answer;
    },
    // Printed first: a batch the provider took is named by the next request;
    // one lost before that arrives again, never not at all.
    printed(batch) { printed.push(batch); },
    close() { queue = queue.then(() => drop(new Error("turn ended"))); },
  };
}
