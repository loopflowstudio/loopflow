(function () {
  "use strict";

  const stampEl = document.getElementById("lf-page");
  if (!stampEl) return;
  let stamp;
  try {
    stamp = JSON.parse(stampEl.textContent || "{}");
  } catch (_) {
    return;
  }

  const pagePath = String(stamp.path || "scratch/page.html");
  const buildId = String(stamp.build_id || "0");
  const work = stamp.work || { kind: "repository" };
  const directory = String(stamp.directory || "");
  const protocol =
    stamp.protocol && typeof stamp.protocol === "object" ? stamp.protocol : null;
  const protocolRevision = Number(protocol && protocol.revision);
  const protocolMinRevision = Number(protocol && protocol.min_revision);
  const storageKey = "lf-page:" + pagePath + ":" + buildId;
  const mainEl = document.querySelector("main.lf-page") || document.querySelector("main") || document.body;

  const desktopHandler =
    window.webkit &&
    window.webkit.messageHandlers &&
    window.webkit.messageHandlers.loopflow;

  let hostMode = desktopHandler ? "desktop" : "clipboard";
  let socket = null;
  let nextReqId = 1;
  const pending = new Map();
  let serverDisconnected = false;

  // Draft & receipt state.
  // Notes (prose & diff/file lines), choices, multiselects, questions, offline actions,
  // and optional browser overall direction enter the reply queue.
  // Executed direct `data-lf` receipts stay separate and only ride as a suffix on ⌘↩.
  function emptyState() {
    return {
      notes: [],
      choices: {},
      multiselects: {},
      questions: {},
      offlineActions: [],
      overall: "",
      receipts: [],
    };
  }

  let state = emptyState();

  function oneLine(text) {
    return String(text || "")
      .replace(/[\r\n\t\x00-\x1f]+/g, " ")
      .replace(/\s+/g, " ")
      .trim();
  }

  function buttonValue(btn) {
    return btn.getAttribute("data-value") || oneLine(btn.textContent);
  }

  function resultTaskId(result) {
    return result && ((result.task && result.task.identifier) || result.identifier);
  }

  function errorMessage(err) {
    return String(err && err.message ? err.message : err);
  }

  function makeButton(label, onClick, className) {
    const btn = document.createElement("button");
    btn.type = "button";
    if (className) btn.className = className;
    btn.textContent = label;
    btn.addEventListener("click", onClick);
    return btn;
  }

  function randomId() {
    if (window.crypto && typeof window.crypto.randomUUID === "function") {
      return window.crypto.randomUUID();
    }
    return "lf-" + Math.random().toString(16).slice(2) + "-" + Date.now().toString(16);
  }

  function shortPath(path) {
    const parts = String(path || "").split("/");
    return parts[parts.length - 1] || path;
  }

  function truncate(text, max) {
    const s = oneLine(text);
    return s.length > max ? s.slice(0, max - 1) + "…" : s;
  }

  function nearestSection(node) {
    let el = node && node.nodeType === 1 ? node : node && node.parentElement;
    while (el && el !== document.body) {
      let prev = el.previousElementSibling;
      while (prev) {
        if (/^H[1-4]$/.test(prev.tagName)) return oneLine(prev.textContent);
        prev = prev.previousElementSibling;
      }
      el = el.parentElement;
    }
    const h1 = document.querySelector("h1");
    return h1 ? oneLine(h1.textContent) : "Top";
  }

  function formatLineSpan(note) {
    const start = Number(note.lineStart);
    const end = Number(note.lineEnd || note.lineStart);
    const span = start === end ? String(start) : start + "–" + end;
    return note.side === "old" ? "old:" + span : span;
  }

  function formatLineAnchor(note) {
    return oneLine(note.path) + ":" + formatLineSpan(note);
  }

  function formatNoteAnchorLabel(note) {
    if (note.kind === "line") return formatLineAnchor(note);
    return "§ " + oneLine(note.section) + ' · "' + truncate(note.quote, 46) + '"';
  }

  function pulseElement(el) {
    if (!el) return;
    try {
      el.scrollIntoView({ behavior: "smooth", block: "center" });
    } catch (_) {}
    el.classList.add("lf-pulse-ring");
    setTimeout(() => el.classList.remove("lf-pulse-ring"), 950);
  }

  function noteAnchorElement(note) {
    if (note.kind === "line") {
      const blocks = mainEl.querySelectorAll(".lf-codeblock[data-path]");
      const attr = note.side === "old" ? "data-old-line" : "data-line";
      const start = Number(note.lineStart);
      for (const block of blocks) {
        if (block.getAttribute("data-path") !== note.path) continue;
        const row = block.querySelector('tr.lf-code-row[' + attr + '="' + start + '"]');
        if (row) return row;
      }
      return null;
    }
    return mainEl.querySelector('.lf-hl[data-lf-note-id="' + CSS.escape(note.id) + '"]');
  }

  function notesInDocumentOrder() {
    if (state.notes.length <= 1) return state.notes.slice();
    const indexed = state.notes.map((note, idx) => ({
      note,
      idx,
      el: noteAnchorElement(note),
    }));
    indexed.sort((a, b) => {
      if (a.el && b.el && a.el !== b.el) {
        const pos = a.el.compareDocumentPosition(b.el);
        if (pos & Node.DOCUMENT_POSITION_FOLLOWING) return -1;
        if (pos & Node.DOCUMENT_POSITION_PRECEDING) return 1;
      }
      if (a.el && !b.el) return -1;
      if (!a.el && b.el) return 1;
      return a.idx - b.idx;
    });
    return indexed.map((entry) => entry.note);
  }

  function deleteNote(id) {
    state.notes = state.notes.filter((n) => n.id !== id);
    if (sidebarEditingId === id) sidebarEditingId = null;
    hideHoverCard();
    refresh(true);
  }

  function queueEntries() {
    const entries = [];
    for (const note of notesInDocumentOrder()) {
      if (note.kind === "line") {
        const anchor = formatLineAnchor(note);
        const ref = "`" + pagePath + "` § " + anchor + " — " + oneLine(note.text);
        entries.push({
          tag: "diff",
          summary: shortPath(note.path) + ":" + formatLineSpan(note) + " — " + oneLine(note.text),
          title: anchor + " — " + oneLine(note.text),
          ref,
          jump: () => pulseElement(noteAnchorElement(note)),
          remove: () => deleteNote(note.id),
        });
      } else {
        const ref =
          "`" +
          pagePath +
          "` § " +
          oneLine(note.section) +
          ' › "' +
          oneLine(note.quote) +
          '" ' +
          oneLine(note.text);
        entries.push({
          tag: "note",
          summary: '"' + truncate(note.quote, 26) + '" — ' + oneLine(note.text),
          title: oneLine(note.section) + ' › "' + oneLine(note.quote) + '" — ' + oneLine(note.text),
          ref,
          jump: () => pulseElement(noteAnchorElement(note)),
          remove: () => deleteNote(note.id),
        });
      }
    }
    for (const [group, entry] of Object.entries(state.choices)) {
      if (!entry || !entry.value) continue;
      const label = oneLine(entry.label || group);
      const val = oneLine(entry.value);
      const ref = "`" + pagePath + "` § " + label + ": " + val;
      entries.push({
        tag: "choice",
        summary: label + ": " + val,
        title: label + ": " + val,
        ref,
        jump: () => pulseElement(document.querySelector('[data-choice="' + CSS.escape(group) + '"]')),
        remove: () => {
          delete state.choices[group];
          refresh(true);
        },
      });
    }
    for (const [group, entry] of Object.entries(state.multiselects)) {
      if (!entry || !Array.isArray(entry.values) || entry.values.length === 0) continue;
      const label = oneLine(entry.label || group);
      const vals = entry.values.map(oneLine).join(", ");
      const ref = "`" + pagePath + "` § " + label + ": " + vals;
      entries.push({
        tag: "choice",
        summary: label + ": " + vals,
        title: label + ": " + vals,
        ref,
        jump: () => pulseElement(document.querySelector('[data-multiselect="' + CSS.escape(group) + '"]')),
        remove: () => {
          delete state.multiselects[group];
          refresh(true);
        },
      });
    }
    for (const [qid, entry] of Object.entries(state.questions)) {
      if (!entry || !oneLine(entry.answer)) continue;
      const label = oneLine(entry.label || qid);
      const ans = oneLine(entry.answer);
      const ref = "`" + pagePath + "` § " + label + ": " + ans;
      entries.push({
        tag: "answer",
        summary: label + ": " + ans,
        title: label + ": " + ans,
        ref,
        jump: () => pulseElement(document.querySelector('[data-question="' + CSS.escape(qid) + '"]')),
        remove: () => {
          delete state.questions[qid];
          refresh(true);
        },
      });
    }
    for (const act of state.offlineActions) {
      const summary = oneLine(act.summary);
      const ref = "`" + pagePath + "` Run: " + summary;
      entries.push({
        tag: "run",
        summary,
        title: "Run: " + summary,
        ref,
        jump: () => {
          const found = actions.find((a) => a.key === act.key);
          if (found) pulseElement(found.slot);
        },
        remove: () => {
          state.offlineActions = state.offlineActions.filter((a) => a.id !== act.id);
          refresh(true);
        },
      });
    }
    return entries;
  }

  function queueItems() {
    const items = queueEntries().map((e) => e.ref);
    const overall = oneLine(state.overall);
    if (!desktopHandler && overall) {
      items.push("`" + pagePath + "` Overall: " + overall);
    }
    return items;
  }

  function formatReplyMessage(items) {
    if (!items.length) return "";
    let message = items.map((ref, idx) => "(" + (idx + 1) + ") " + oneLine(ref)).join(" ");
    const unreported = state.receipts.filter((r) => !r.reported);
    if (unreported.length > 0) {
      const ran = unreported
        .map((r) => oneLine(r.kind + (r.summary ? " → " + r.summary : "")))
        .join("; ");
      message += " [already ran: " + ran + "]";
    }
    return oneLine(message);
  }

  function saveState() {
    if (desktopHandler) {
      const items = queueItems();
      try {
        desktopHandler.postMessage({
          type: "sync",
          path: pagePath,
          build_id: buildId,
          state: state,
          items: items,
        });
      } catch (_) {}
      return;
    }
    try {
      window.localStorage.setItem(storageKey, JSON.stringify(state));
    } catch (_) {}
  }

  function loadBrowserState() {
    if (desktopHandler) return;
    try {
      const raw = window.localStorage.getItem(storageKey);
      if (!raw) return;
      const parsed = JSON.parse(raw);
      if (parsed && typeof parsed === "object") {
        applyState(parsed);
      }
    } catch (_) {}
  }

  // `persist` marks an edit: the last send's sentence no longer describes the queue.
  function refresh(persist) {
    if (persist) {
      stripStatusText = "";
      stripStatusKind = "";
    }
    syncControlsFromState();
    renderHighlights();
    renderSidebar();
    if (persist) saveState();
    renderDock();
  }

  function applyState(loaded) {
    if (!loaded || typeof loaded !== "object") {
      state = emptyState();
    } else {
      state = {
        notes: Array.isArray(loaded.notes) ? loaded.notes : [],
        choices: loaded.choices && typeof loaded.choices === "object" ? loaded.choices : {},
        multiselects:
          loaded.multiselects && typeof loaded.multiselects === "object" ? loaded.multiselects : {},
        questions: loaded.questions && typeof loaded.questions === "object" ? loaded.questions : {},
        offlineActions: Array.isArray(loaded.offlineActions) ? loaded.offlineActions : [],
        overall: typeof loaded.overall === "string" ? loaded.overall : "",
        receipts: Array.isArray(loaded.receipts) ? loaded.receipts : [],
      };
    }
    refresh(false);
  }

  // Desktop calls this with `null` after `ReviewComments.insert(into:)` sends the page's items;
  // Swift has already updated its stored page state, so don't echo a redundant `sync` back.
  window.__lfDesktopApplyState = function (loaded) {
    if (loaded) applyState(loaded);
    else clearDrafts({ markReported: true, persist: false });
  };

  // --- Loopback WebSocket carrier ---

  function loopbackWsUrl() {
    if (window.location.protocol !== "http:" && window.location.protocol !== "https:") {
      return null;
    }
    const parts = window.location.pathname.split("/").filter(Boolean);
    if (parts.length < 3 || parts[1] !== "f") return null;
    const token = parts[0];
    const wsProto = window.location.protocol === "https:" ? "wss:" : "ws:";
    return wsProto + "//" + window.location.host + "/" + token + "/machine";
  }

  function markDisconnected(reason) {
    hostMode = "clipboard";
    serverDisconnected = true;
    showBanner(reason || "Loopflow server disconnected — run lf page open " + pagePath);
    renderDock();
  }

  function connectLoopback() {
    if (desktopHandler) return;
    const url = loopbackWsUrl();
    if (!url) return;
    if (!Number.isInteger(protocolRevision) || !Number.isInteger(protocolMinRevision)) {
      markDisconnected("Review page is missing server protocol version — rebuild with lf page build");
      return;
    }
    let ws;
    try {
      ws = new WebSocket(url);
    } catch (_) {
      markDisconnected("");
      return;
    }
    let handshakeError = "";
    let closingForUnload = false;
    ws.addEventListener("open", () => {
      ws.send(
        JSON.stringify({
          type: "hello",
          revision: protocolRevision,
          min_revision: protocolMinRevision,
          client: {
            kind: "page",
            platform: "browser",
            name: "review-page",
            version: "1",
          },
        })
      );
    });
    ws.addEventListener("message", (event) => {
      let msg;
      try {
        msg = JSON.parse(event.data);
      } catch (_) {
        return;
      }
      if (msg.type === "welcome") {
        socket = ws;
        hostMode = "loopback";
        serverDisconnected = false;
        hideBanner();
        renderDock();
        return;
      }
      if (msg.type === "error" && !msg.id) {
        handshakeError = msg.message || msg.code || "Loopflow server rejected connection";
        showBanner(handshakeError);
        return;
      }
      if ((msg.type === "response" || msg.type === "error") && msg.id && pending.has(msg.id)) {
        const entry = pending.get(msg.id);
        pending.delete(msg.id);
        if (msg.type === "response") entry.resolve(msg.result);
        else entry.reject(new Error(msg.message || msg.code || "Server error"));
      }
    });
    ws.addEventListener("close", () => {
      if (socket === ws) socket = null;
      for (const [, entry] of pending) {
        entry.reject(new Error("Connection closed"));
      }
      pending.clear();
      if (closingForUnload) return;
      markDisconnected(handshakeError);
    });
    window.addEventListener("pagehide", () => {
      closingForUnload = true;
      try {
        ws.close();
      } catch (_) {}
    });
  }

  function sendOperation(operation, requestId) {
    if (desktopHandler) {
      try {
        return Promise.resolve(
          desktopHandler.postMessage({
            type: "operation",
            operation: operation,
            request_id: requestId || null,
          })
        ).then((result) => {
          if (result && result.error) throw new Error(result.error);
          return result && result.result !== undefined ? result.result : result;
        });
      } catch (err) {
        return Promise.reject(err);
      }
    }
    if (!socket || socket.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error("Loopflow server is not connected"));
    }
    return new Promise((resolve, reject) => {
      const id = "req-" + nextReqId++;
      pending.set(id, { resolve, reject });
      socket.send(
        JSON.stringify({
          type: "request",
          id: id,
          operation: operation,
          select: null,
          sender: null,
          request_id: requestId || null,
        })
      );
    });
  }

  // --- Banner ---

  let bannerEl = null;
  function showBanner(text) {
    if (!bannerEl) {
      bannerEl = document.createElement("div");
      bannerEl.className = "lf-banner";
      mainEl.insertBefore(bannerEl, mainEl.firstChild);
    }
    bannerEl.textContent = text;
    bannerEl.hidden = false;
  }

  function hideBanner() {
    if (bannerEl) bannerEl.hidden = true;
  }

  // --- Controls ---

  function wireControls() {
    // Toggle line notes
    document.querySelectorAll("[data-notes-toggle]").forEach((btn) => {
      btn.addEventListener("click", () => {
        const block = btn.closest(".lf-codeblock");
        if (!block) return;
        const hidden = block.classList.toggle("lf-notes-hidden");
        btn.textContent = hidden ? "Show line notes" : "Hide line notes";
      });
    });

    // Single select (`data-choice`)
    document.querySelectorAll("[data-choice]").forEach((groupEl) => {
      const group = groupEl.getAttribute("data-choice");
      const label = groupEl.getAttribute("data-label") || group;
      groupEl.querySelectorAll("button").forEach((btn) => {
        btn.addEventListener("click", () => {
          const value = buttonValue(btn);
          if (state.choices[group] && state.choices[group].value === value) {
            delete state.choices[group];
          } else {
            state.choices[group] = { label, value };
          }
          refresh(true);
        });
      });
    });

    // Multi select (`data-multiselect`)
    document.querySelectorAll("[data-multiselect]").forEach((groupEl) => {
      const group = groupEl.getAttribute("data-multiselect");
      const label = groupEl.getAttribute("data-label") || group;
      groupEl.querySelectorAll("button").forEach((btn) => {
        btn.addEventListener("click", () => {
          const value = buttonValue(btn);
          const current = (state.multiselects[group] && state.multiselects[group].values) || [];
          const idx = current.indexOf(value);
          const next = idx >= 0 ? current.filter((v) => v !== value) : current.concat([value]);
          if (next.length === 0) delete state.multiselects[group];
          else state.multiselects[group] = { label, values: next };
          refresh(true);
        });
      });
    });

    // Questions (`data-question`)
    document.querySelectorAll("[data-question]").forEach((box) => {
      const qid = box.getAttribute("data-question");
      const label = box.getAttribute("data-label") || qid;
      const input = box.querySelector("textarea, input");
      if (!input) return;
      input.addEventListener("input", () => {
        const answer = input.value;
        if (!oneLine(answer)) delete state.questions[qid];
        else state.questions[qid] = { label, answer };
        stripStatusText = "";
        stripStatusKind = "";
        saveState();
        renderDock();
      });
    });

    // Direct actions (`data-lf`)
    document.querySelectorAll("[data-lf]").forEach((btn, idx) => {
      let template;
      try {
        template = JSON.parse(btn.getAttribute("data-lf") || "{}");
      } catch (_) {
        return;
      }
      const action = { key: btn.getAttribute("data-lf-id") || "action-" + idx, btn, template, slot: btn };
      actions.push(action);
      btn.addEventListener("click", () => {
        openActionConfirm(action);
      });
    });
  }

  // Each `data-lf` button and the element now standing in its place.
  const actions = [];

  function showInSlot(action, el) {
    if (action.slot !== el) action.slot.replaceWith(el);
    action.slot = el;
  }

  function actionNote(className, text) {
    const el = document.createElement("span");
    el.className = className;
    el.textContent = text;
    return el;
  }

  // Receipts and offline-queued actions outlive reloads; an action with neither shows its button.
  function syncActionsFromState() {
    for (const action of actions) {
      const receipt = state.receipts.find((r) => r.key === action.key);
      const offline = state.offlineActions.find((a) => a.key === action.key);
      if (receipt) showInSlot(action, actionNote("lf-action-receipt", receipt.text || receipt.summary));
      else if (offline) showInSlot(action, actionNote("lf-action-summary", "Offline — added to reply"));
      else if (action.slot.classList.contains("lf-action-receipt") || action.slot.classList.contains("lf-action-summary")) {
        showInSlot(action, action.btn);
      }
    }
  }

  function syncControlsFromState() {
    syncActionsFromState();
    document.querySelectorAll("[data-choice]").forEach((groupEl) => {
      const group = groupEl.getAttribute("data-choice");
      const selected = state.choices[group] && state.choices[group].value;
      groupEl.querySelectorAll("button").forEach((btn) => {
        const active = selected === buttonValue(btn);
        btn.classList.toggle("is-selected", active);
        btn.setAttribute("aria-pressed", active ? "true" : "false");
      });
    });
    document.querySelectorAll("[data-multiselect]").forEach((groupEl) => {
      const group = groupEl.getAttribute("data-multiselect");
      const values = (state.multiselects[group] && state.multiselects[group].values) || [];
      groupEl.querySelectorAll("button").forEach((btn) => {
        const active = values.includes(buttonValue(btn));
        btn.classList.toggle("is-selected", active);
        btn.setAttribute("aria-pressed", active ? "true" : "false");
      });
    });
    document.querySelectorAll("[data-question]").forEach((box) => {
      const qid = box.getAttribute("data-question");
      const input = box.querySelector("textarea, input");
      if (!input) return;
      const saved = state.questions[qid] ? state.questions[qid].answer : "";
      if (input.value !== saved) input.value = saved;
    });
  }

  // --- Direct `data-lf` actions ---

  function summarizeOperation(op) {
    const parts = [op.kind || "operation"];
    for (const [k, v] of Object.entries(op)) {
      if (k === "kind" || k === "directory" || k === "request_id" || v === null || v === false || v === "") {
        continue;
      }
      if (Array.isArray(v) && v.length === 0) continue;
      parts.push(k + "=" + (typeof v === "string" ? v : JSON.stringify(v)));
    }
    return parts.join(" ");
  }

  function materializeOperation(template, requestId) {
    const copy = JSON.parse(JSON.stringify(template));
    if (copy.request_id === "__LF_REQUEST_ID__") {
      copy.request_id = requestId;
    }
    return copy;
  }

  function formatReceipt(kind, result) {
    if (result && typeof result === "object") {
      const taskId = resultTaskId(result);
      if (kind === "task.create" && taskId) return "Filed " + taskId;
      if (result.delivery && result.delivery.sentence) return result.delivery.sentence;
      if (taskId) return kind + " → " + taskId;
      if (result.sentence) return result.sentence;
    }
    return "Ran " + kind;
  }

  function openActionConfirm(action) {
    const template = action.template;
    if (hostMode === "clipboard") {
      state.offlineActions.push({ id: randomId(), key: action.key, summary: summarizeOperation(template) });
      refresh(true);
      return;
    }

    const requestId = randomId();
    const wrap = document.createElement("span");
    wrap.className = "lf-action-confirm";

    const summary = actionNote("lf-action-summary", summarizeOperation(template));
    const runBtn = makeButton("Run", execute, "lf-btn-accent");
    const cancelBtn = makeButton("Cancel", () => showInSlot(action, action.btn));

    wrap.append(summary, runBtn, cancelBtn);
    showInSlot(action, wrap);

    function execute() {
      showInSlot(action, wrap);
      runBtn.disabled = true;
      runBtn.textContent = "Running…";
      const op = materializeOperation(template, requestId);
      sendOperation(op, requestId).then(
        (result) => {
          const text = formatReceipt(op.kind, result);
          state.receipts.push({ key: action.key, kind: op.kind, summary: resultTaskId(result) || text, text });
          refresh(true);
        },
        (err) => {
          const errWrap = document.createElement("span");
          errWrap.className = "lf-action-error";
          errWrap.append(actionNote("", errorMessage(err)), makeButton("Retry", execute));
          showInSlot(action, errWrap);
        }
      );
    }
  }

  // --- Cross-element prose highlight anchoring ---

  const EXCLUDED_TEXT_SELECTOR =
    "button, textarea, input, script, style, .lf-gutter-plus, .lf-ln, .lf-sign, " +
    ".lf-float-pill, .lf-float-composer, .lf-hover-card, .lf-sidebar, .lf-dock, " +
    ".lf-comments-toggle, .lf-banner, .lf-action-confirm, .lf-action-receipt, .lf-action-error";

  const BLOCK_TAGS = new Set([
    "P", "DIV", "LI", "TR", "TD", "TH", "H1", "H2", "H3", "H4", "H5", "H6",
    "PRE", "BLOCKQUOTE", "SECTION", "ARTICLE", "ASIDE", "HEADER", "FOOTER", "DT", "DD",
  ]);

  function nearestBlock(node) {
    let el = node && node.nodeType === 1 ? node : node && node.parentElement;
    while (el && el !== mainEl) {
      if (BLOCK_TAGS.has(el.tagName)) return el;
      el = el.parentElement;
    }
    return mainEl;
  }

  function isExcludedTextNode(node) {
    const parent = node.parentElement;
    if (!parent) return true;
    if (parent.closest(EXCLUDED_TEXT_SELECTOR)) return true;
    if (parent.closest("[hidden]")) return true;
    if (parent.closest(".lf-notes-hidden .lf-line-note")) return true;
    return false;
  }

  function buildNormalizedIndex(root) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    let flat = "";
    const starts = [];
    const ends = [];
    let lastWasSpace = true;
    let lastBlock = null;
    let node;
    while ((node = walker.nextNode())) {
      if (!node.nodeValue || isExcludedTextNode(node)) continue;
      const block = nearestBlock(node);
      if (lastBlock && block !== lastBlock && !lastWasSpace) {
        flat += " ";
        starts.push(null);
        ends.push(null);
        lastWasSpace = true;
      }
      lastBlock = block;
      const val = node.nodeValue;
      for (let i = 0; i < val.length; i++) {
        const ch = val[i];
        if (/\s/.test(ch)) {
          if (!lastWasSpace) {
            flat += " ";
            starts.push({ node, offset: i });
            ends.push({ node, offset: i + 1 });
            lastWasSpace = true;
          }
        } else {
          flat += ch;
          starts.push({ node, offset: i });
          ends.push({ node, offset: i + 1 });
          lastWasSpace = false;
        }
      }
    }
    while (flat.length > 0 && flat[flat.length - 1] === " ") {
      flat = flat.slice(0, -1);
      starts.pop();
      ends.pop();
    }
    return { flat, starts, ends };
  }

  function findOccurrenceForRange(range, needle) {
    if (!needle) return 0;
    const index = buildNormalizedIndex(mainEl);
    let rangeFlatPos = -1;
    for (let i = 0; i < index.starts.length; i++) {
      const p = index.starts[i];
      if (!p) continue;
      try {
        if (range.comparePoint(p.node, p.offset) === 0) {
          rangeFlatPos = i;
          break;
        }
      } catch (_) {}
    }
    if (rangeFlatPos < 0) return 0;
    let count = 0;
    let from = 0;
    while (from < rangeFlatPos) {
      const hit = index.flat.indexOf(needle, from);
      if (hit < 0 || hit >= rangeFlatPos) break;
      count++;
      from = hit + 1;
    }
    return count;
  }

  function findNormalizedRange(needle, occurrence) {
    if (!needle) return null;
    const index = buildNormalizedIndex(mainEl);
    let hit = -1;
    let from = 0;
    const targetOcc = Math.max(0, Number(occurrence) || 0);
    for (let occ = 0; occ <= targetOcc; occ++) {
      const next = index.flat.indexOf(needle, from);
      if (next < 0) break;
      hit = next;
      from = next + 1;
    }
    if (hit < 0) return null;
    let s = hit;
    let e = hit + needle.length - 1;
    while (s <= e && !index.starts[s]) s++;
    while (e >= s && !index.ends[e]) e--;
    if (s > e || !index.starts[s] || !index.ends[e]) return null;
    try {
      const range = document.createRange();
      range.setStart(index.starts[s].node, index.starts[s].offset);
      range.setEnd(index.ends[e].node, index.ends[e].offset);
      return range;
    } catch (_) {
      return null;
    }
  }

  function wrapRange(range, noteId) {
    const walker = document.createTreeWalker(mainEl, NodeFilter.SHOW_TEXT);
    const hits = [];
    let node;
    while ((node = walker.nextNode())) {
      if (!node.nodeValue || isExcludedTextNode(node)) continue;
      let intersects = false;
      try {
        intersects = range.intersectsNode(node);
      } catch (_) {
        continue;
      }
      if (!intersects) continue;
      const start = node === range.startContainer ? range.startOffset : 0;
      const end = node === range.endContainer ? range.endOffset : node.nodeValue.length;
      if (end > start && node.nodeValue.slice(start, end).trim()) {
        hits.push({ node, start, end });
      }
    }
    for (let i = hits.length - 1; i >= 0; i--) {
      const { node: tn, start, end } = hits[i];
      const text = tn.nodeValue;
      const before = text.slice(0, start);
      const mid = text.slice(start, end);
      const after = text.slice(end);
      const frag = document.createDocumentFragment();
      if (before) frag.appendChild(document.createTextNode(before));
      const span = document.createElement("span");
      span.className = "lf-hl";
      span.setAttribute("data-lf-note-id", noteId);
      span.textContent = mid;
      frag.appendChild(span);
      if (after) frag.appendChild(document.createTextNode(after));
      if (tn.parentNode) tn.parentNode.replaceChild(frag, tn);
    }
  }

  function unwrapAllHighlights() {
    const spans = Array.from(mainEl.querySelectorAll("span.lf-hl[data-lf-note-id]"));
    const parents = new Set();
    for (let i = spans.length - 1; i >= 0; i--) {
      const span = spans[i];
      const parent = span.parentNode;
      if (!parent) continue;
      parents.add(parent);
      while (span.firstChild) parent.insertBefore(span.firstChild, span);
      parent.removeChild(span);
    }
    for (const parent of parents) {
      if (parent.isConnected) parent.normalize();
    }
  }

  function highlightSignature() {
    return JSON.stringify(
      state.notes.map((n) =>
        n.kind === "line"
          ? [n.id, "line", n.path, n.side, n.lineStart, n.lineEnd]
          : [n.id, "prose", n.match || n.quote, n.occurrence || 0]
      )
    );
  }

  let lastHighlightSig = "";

  function renderHighlights() {
    const sig = highlightSignature();
    if (sig === lastHighlightSig) return;
    lastHighlightSig = sig;

    unwrapAllHighlights();

    // Clear existing line-note markers on diff/file rows
    mainEl.querySelectorAll("tr.lf-code-row.lf-row-has-note").forEach((tr) => {
      tr.classList.remove("lf-row-has-note");
      tr.removeAttribute("data-lf-line-note-ids");
    });

    for (const note of state.notes) {
      if (note.kind === "line") {
        markLineNoteRows(note);
      } else {
        const needle = oneLine(note.match || note.quote);
        const range = findNormalizedRange(needle, note.occurrence || 0);
        if (range) wrapRange(range, note.id);
      }
    }
  }

  function rowsForLineNote(note) {
    const matched = [];
    const blocks = mainEl.querySelectorAll(".lf-codeblock[data-path]");
    const attr = note.side === "old" ? "data-old-line" : "data-line";
    const start = Number(note.lineStart);
    const end = Number(note.lineEnd || note.lineStart);
    for (const block of blocks) {
      if (block.getAttribute("data-path") !== note.path) continue;
      block.querySelectorAll("tr.lf-code-row[" + attr + "]").forEach((tr) => {
        const ln = Number(tr.getAttribute(attr));
        if (ln >= start && ln <= end) matched.push(tr);
      });
    }
    return matched;
  }

  function markLineNoteRows(note) {
    for (const tr of rowsForLineNote(note)) {
      tr.classList.add("lf-row-has-note");
      const existing = tr.getAttribute("data-lf-line-note-ids");
      const list = existing ? existing.split(",") : [];
      if (!list.includes(note.id)) list.push(note.id);
      tr.setAttribute("data-lf-line-note-ids", list.join(","));
    }
  }

  // --- Floating selection pill, composer, and hover cards (1c) ---

  let floatPill = null;
  let floatComposer = null;
  let pendingSelection = null; // { range, rect, section, quote, match }
  let hoverCardEl = null;
  let hoverHideTimer = null;

  function clearSelectingRows() {
    mainEl.querySelectorAll("tr.lf-code-row.lf-row-selecting").forEach((tr) => {
      tr.classList.remove("lf-row-selecting");
    });
  }

  function dismissFloatPill() {
    if (floatPill) {
      floatPill.remove();
      floatPill = null;
    }
  }

  function dismissFloatComposer() {
    if (floatComposer) {
      floatComposer.remove();
      floatComposer = null;
    }
    clearSelectingRows();
  }

  function dismissFloats() {
    dismissFloatPill();
    dismissFloatComposer();
  }

  function hideHoverCard() {
    clearTimeout(hoverHideTimer);
    if (hoverCardEl) {
      hoverCardEl.remove();
      hoverCardEl = null;
    }
  }

  function clampFixedBox(el, rect, preferAbove) {
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const dockAllowance = desktopHandler ? 12 : 96;
    let top;
    if (preferAbove) {
      top = rect.top - h - 8;
      if (top < 8) top = Math.min(window.innerHeight - h - dockAllowance, rect.bottom + 8);
    } else {
      top = rect.bottom + 8;
      if (top + h > window.innerHeight - dockAllowance) {
        top = Math.max(8, rect.top - h - 8);
      }
    }
    top = Math.max(8, Math.min(window.innerHeight - h - 8, top));
    const centered = preferAbove ? rect.left + rect.width / 2 - w / 2 : rect.left;
    const left = Math.max(8, Math.min(window.innerWidth - w - 8, centered));
    el.style.top = top + "px";
    el.style.left = left + "px";
  }

  function showFloatPill(rect, buttons) {
    dismissFloatPill();
    floatPill = document.createElement("div");
    floatPill.className = "lf-float-pill";
    buttons.forEach((b, idx) => {
      if (idx > 0) {
        const sep = document.createElement("span");
        sep.className = "lf-float-pill-sep";
        floatPill.appendChild(sep);
      }
      const btn = document.createElement("button");
      btn.type = "button";
      btn.textContent = b.label;
      btn.addEventListener("mousedown", (e) => e.preventDefault());
      btn.addEventListener("click", () => {
        dismissFloatPill();
        b.onClick();
      });
      floatPill.appendChild(btn);
    });
    document.body.appendChild(floatPill);
    clampFixedBox(floatPill, rect, true);
  }

  function saveProseNote(selInfo, text) {
    const needle = selInfo.match;
    const occurrence = findOccurrenceForRange(selInfo.range, needle);
    state.notes.push({
      id: randomId(),
      kind: "prose",
      section: selInfo.section,
      quote: selInfo.quote.slice(0, 140),
      match: needle,
      occurrence,
      text: oneLine(text),
    });
    pendingSelection = null;
    try {
      window.getSelection().removeAllRanges();
    } catch (_) {}
    refresh(true);
  }

  function openFloatingComposer(rect, opts) {
    dismissFloats();
    hideHoverCard();

    if (opts.selectedRows) {
      for (const tr of opts.selectedRows) tr.classList.add("lf-row-selecting");
    }

    floatComposer = document.createElement("div");
    floatComposer.className = "lf-float-composer";

    const quoteEl = document.createElement("div");
    quoteEl.className = "lf-anchor-quote";
    quoteEl.textContent = opts.anchorLabel;

    const ta = document.createElement("textarea");
    ta.rows = 2;
    ta.placeholder = opts.placeholder || "Add comment… (⌘↩ to save)";
    ta.value = opts.initialText || "";

    const acts = document.createElement("div");
    acts.className = "lf-float-composer-actions";

    function commit() {
      const val = oneLine(ta.value);
      if (!val) return;
      dismissFloatComposer();
      opts.onSave(val);
    }

    ta.addEventListener("keydown", (e) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
        e.preventDefault();
        e.stopPropagation();
        commit();
      } else if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        dismissFloatComposer();
      }
    });

    acts.append(
      makeButton("Cancel", dismissFloatComposer, "lf-btn-quiet"),
      makeButton("Save ⌘↩", commit, "lf-btn-accent")
    );
    floatComposer.append(quoteEl, ta, acts);
    document.body.appendChild(floatComposer);
    clampFixedBox(floatComposer, rect, false);
    ta.focus();
    ta.setSelectionRange(ta.value.length, ta.value.length);
  }

  function showHoverCard(targetEl, note) {
    if (floatComposer) return;
    clearTimeout(hoverHideTimer);
    hideHoverCard();

    hoverCardEl = document.createElement("div");
    hoverCardEl.className = "lf-hover-card";

    const quoteEl = document.createElement("div");
    quoteEl.className = "lf-anchor-quote";
    quoteEl.textContent = formatNoteAnchorLabel(note);

    const body = document.createElement("div");
    body.className = "lf-hover-card-body";
    body.textContent = note.text;

    const foot = document.createElement("div");
    foot.className = "lf-hover-card-foot";
    const editBtn = makeButton("Edit", () => {
      const r = targetEl.getBoundingClientRect();
      hideHoverCard();
      openFloatingComposer(r, {
        anchorLabel: formatNoteAnchorLabel(note),
        initialText: note.text,
        onSave: (val) => {
          note.text = val;
          refresh(true);
        },
      });
    }, "lf-btn-quiet");
    const delBtn = makeButton("Delete", () => {
      deleteNote(note.id);
    }, "lf-btn-quiet");

    foot.append(editBtn, delBtn);
    hoverCardEl.append(quoteEl, body, foot);
    document.body.appendChild(hoverCardEl);
    clampFixedBox(hoverCardEl, targetEl.getBoundingClientRect(), false);

    hoverCardEl.addEventListener("mouseenter", () => clearTimeout(hoverHideTimer));
    hoverCardEl.addEventListener("mouseleave", () => {
      hoverHideTimer = setTimeout(hideHoverCard, 200);
    });
  }

  function wireSelectionNotes() {
    document.addEventListener("mouseup", (event) => {
      if (event.target.closest(".lf-float-pill, .lf-float-composer, .lf-hover-card, .lf-sidebar, .lf-dock, .lf-comments-toggle")) {
        return;
      }
      if (event.target.closest("button, textarea, input, .lf-action-confirm, td.lf-ln")) {
        return;
      }

      const sel = window.getSelection();
      if (!sel || sel.isCollapsed) {
        pendingSelection = null;
        dismissFloatPill();
        return;
      }
      const quote = oneLine(sel.toString());
      if (!quote) {
        pendingSelection = null;
        dismissFloatPill();
        return;
      }
      const range = sel.getRangeAt(0).cloneRange();
      const containerEl =
        range.commonAncestorContainer.nodeType === 1
          ? range.commonAncestorContainer
          : range.commonAncestorContainer.parentElement;
      if (!containerEl || !mainEl.contains(containerEl)) {
        pendingSelection = null;
        dismissFloatPill();
        return;
      }
      const rect = range.getBoundingClientRect();
      const section = nearestSection(containerEl);
      const selInfo = {
        range,
        rect,
        section,
        quote: quote.slice(0, 140),
        match: quote.slice(0, 240),
      };
      pendingSelection = selInfo;

      showFloatPill(rect, [
        {
          label: "Comment (c)",
          onClick: () => {
            openFloatingComposer(selInfo.rect, {
              anchorLabel: "§ " + selInfo.section + ' · "' + truncate(selInfo.quote, 46) + '"',
              onSave: (val) => saveProseNote(selInfo, val),
            });
          },
        },
        {
          label: "Flag",
          onClick: () => saveProseNote(selInfo, "Flagged for follow-up"),
        },
        {
          label: "Copy",
          onClick: () => {
            copyText(quote).catch(() => {});
            pendingSelection = null;
            try {
              window.getSelection().removeAllRanges();
            } catch (_) {}
          },
        },
      ]);
    });

    document.addEventListener("mousedown", (event) => {
      if (
        floatComposer &&
        !floatComposer.contains(event.target) &&
        !event.target.closest(".lf-float-pill, .lf-gutter-plus, td.lf-ln")
      ) {
        dismissFloatComposer();
      }
    });

    // Hover & click on warm-underlined prose highlights and annotated code rows
    mainEl.addEventListener("mouseover", (event) => {
      const hl = event.target.closest(".lf-hl[data-lf-note-id]");
      if (hl) {
        const id = hl.getAttribute("data-lf-note-id");
        const note = state.notes.find((n) => n.id === id);
        if (note) showHoverCard(hl, note);
        return;
      }
      const lnCell = event.target.closest("tr.lf-code-row.lf-row-has-note td.lf-ln");
      if (lnCell) {
        const tr = lnCell.closest("tr.lf-code-row");
        const ids = (tr.getAttribute("data-lf-line-note-ids") || "").split(",").filter(Boolean);
        const note = ids.length ? state.notes.find((n) => n.id === ids[0]) : null;
        if (note) showHoverCard(lnCell, note);
      }
    });

    mainEl.addEventListener("mouseout", (event) => {
      if (event.target.closest(".lf-hl[data-lf-note-id], tr.lf-code-row.lf-row-has-note td.lf-ln")) {
        hoverHideTimer = setTimeout(hideHoverCard, 220);
      }
    });

    mainEl.addEventListener("click", (event) => {
      const hl = event.target.closest(".lf-hl[data-lf-note-id]");
      if (!hl) return;
      const sel = window.getSelection();
      if (sel && !sel.isCollapsed) return;
      openSidebar(hl.getAttribute("data-lf-note-id"));
    });
  }

  // --- Diff & file gutter `+` and Shift+click line ranges (2a) ---

  let lastGutterAnchor = null; // { block, path, side, lineNo, oldLine }

  function rowLineInfo(tr) {
    const newAttr = tr.getAttribute("data-line");
    const oldAttr = tr.getAttribute("data-old-line");
    if (newAttr !== null && newAttr !== "") {
      return {
        side: "new",
        lineNo: Number(newAttr),
        oldLine: oldAttr !== null && oldAttr !== "" ? Number(oldAttr) : null,
      };
    }
    if (oldAttr !== null && oldAttr !== "") {
      return { side: "old", lineNo: Number(oldAttr), oldLine: Number(oldAttr) };
    }
    return null;
  }

  function wireCodeBlocks() {
    mainEl.querySelectorAll(".lf-codeblock[data-path]").forEach((block) => {
      const path = block.getAttribute("data-path") || "";
      if (!path) return;
      block.querySelectorAll("tr.lf-code-row").forEach((tr) => {
        const info = rowLineInfo(tr);
        if (!info) return;
        const firstLn = tr.querySelector("td.lf-ln");
        if (!firstLn || firstLn.querySelector(".lf-gutter-plus")) return;
        const label = info.side === "old" ? "old line " + info.lineNo : "line " + info.lineNo;
        const plus = document.createElement("button");
        plus.type = "button";
        plus.className = "lf-gutter-plus";
        plus.textContent = "+";
        plus.title = "Comment on " + label + " (Shift+click for range)";
        plus.setAttribute("aria-label", "Comment on " + path + " " + label);
        firstLn.insertBefore(plus, firstLn.firstChild);
      });

      block.addEventListener("mousedown", (event) => {
        if (event.shiftKey && event.target.closest("td.lf-ln")) {
          event.preventDefault();
        }
      });

      block.addEventListener("click", (event) => {
        const trigger = event.target.closest(".lf-gutter-plus, td.lf-ln");
        if (!trigger) return;
        const tr = trigger.closest("tr.lf-code-row");
        if (!tr) return;
        const info = rowLineInfo(tr);
        if (!info) return;
        event.preventDefault();
        dismissFloatPill();
        hideHoverCard();

        let rangeSide = info.side;
        let lineStart = info.lineNo;
        let lineEnd = info.lineNo;

        if (event.shiftKey && lastGutterAnchor && lastGutterAnchor.block === block) {
          if (lastGutterAnchor.side === info.side) {
            rangeSide = info.side;
            lineStart = Math.min(lastGutterAnchor.lineNo, info.lineNo);
            lineEnd = Math.max(lastGutterAnchor.lineNo, info.lineNo);
          } else if (info.side === "old" && lastGutterAnchor.oldLine !== null) {
            rangeSide = "old";
            lineStart = Math.min(lastGutterAnchor.oldLine, info.lineNo);
            lineEnd = Math.max(lastGutterAnchor.oldLine, info.lineNo);
          } else if (lastGutterAnchor.side === "old" && info.oldLine !== null) {
            rangeSide = "old";
            lineStart = Math.min(lastGutterAnchor.lineNo, info.oldLine);
            lineEnd = Math.max(lastGutterAnchor.lineNo, info.oldLine);
          }
        }
        lastGutterAnchor = { block, path, side: info.side, lineNo: info.lineNo, oldLine: info.oldLine };

        const previewNote = { kind: "line", path, side: rangeSide, lineStart, lineEnd };
        const selectedRows = rowsForLineNote(previewNote);
        const anchorRow = selectedRows[selectedRows.length - 1] || tr;
        const anchorLabel = formatLineAnchor(previewNote);
        const rowRect = anchorRow.getBoundingClientRect();
        const codeCell = anchorRow.lastElementChild;
        const codeLeft = codeCell ? codeCell.getBoundingClientRect().left : rowRect.left + 110;
        const composerRect = {
          top: rowRect.top,
          bottom: rowRect.bottom,
          left: codeLeft + 12,
          right: rowRect.right,
          width: Math.max(0, rowRect.right - codeLeft - 12),
        };

        openFloatingComposer(composerRect, {
          anchorLabel,
          placeholder:
            lineStart === lineEnd
              ? "Add comment… (Shift+click another line for range, ⌘↩ to save)"
              : "Add comment… (⌘↩ to save)",
          selectedRows: selectedRows.length ? selectedRows : [tr],
          onSave: (val) => {
            state.notes.push({
              id: randomId(),
              kind: "line",
              path,
              side: rangeSide,
              lineStart,
              lineEnd,
              section: nearestSection(block),
              text: val,
            });
            refresh(true);
          },
        });
      });
    });
  }

  // --- Collapsible slide-over Comments sidebar (1c) ---

  let commentsToggleBtn = null;
  let sidebarEl = null;
  let sidebarTitleEl = null;
  let sidebarListEl = null;
  let sidebarOpen = false;
  let sidebarEditingId = null;

  function ensureSidebar() {
    if (sidebarEl) return;

    commentsToggleBtn = document.createElement("button");
    commentsToggleBtn.type = "button";
    commentsToggleBtn.className = "lf-comments-toggle";
    commentsToggleBtn.addEventListener("click", () => {
      sidebarOpen = !sidebarOpen;
      renderSidebar();
    });
    document.body.appendChild(commentsToggleBtn);

    sidebarEl = document.createElement("aside");
    sidebarEl.className = "lf-sidebar";
    sidebarEl.setAttribute("aria-label", "Comments sidebar");

    const head = document.createElement("div");
    head.className = "lf-sidebar-head";
    sidebarTitleEl = document.createElement("span");
    const closeBtn = makeButton("Close", () => {
      sidebarOpen = false;
      sidebarEditingId = null;
      renderSidebar();
    }, "lf-btn-quiet");
    head.append(sidebarTitleEl, closeBtn);

    sidebarListEl = document.createElement("div");
    sidebarListEl.className = "lf-sidebar-list";

    sidebarEl.append(head, sidebarListEl);
    document.body.appendChild(sidebarEl);
  }

  function openSidebar(focusNoteId) {
    ensureSidebar();
    sidebarOpen = true;
    renderSidebar();
    if (focusNoteId && sidebarListEl) {
      const card = sidebarListEl.querySelector('[data-lf-sidebar-note="' + CSS.escape(focusNoteId) + '"]');
      if (card) pulseElement(card);
    }
  }

  function renderSidebar() {
    ensureSidebar();
    const ordered = notesInDocumentOrder();
    const count = ordered.length;

    commentsToggleBtn.innerHTML = "";
    const labelSpan = document.createElement("span");
    labelSpan.textContent = "Comments";
    commentsToggleBtn.appendChild(labelSpan);
    if (count > 0) {
      const badge = document.createElement("span");
      badge.className = "lf-comments-count";
      badge.textContent = String(count);
      commentsToggleBtn.appendChild(badge);
    }
    commentsToggleBtn.classList.toggle("has-notes", count > 0);
    commentsToggleBtn.setAttribute("aria-expanded", sidebarOpen ? "true" : "false");

    sidebarEl.classList.toggle("is-open", sidebarOpen);
    sidebarTitleEl.textContent = count > 0 ? "Comments (" + count + ")" : "Comments";

    sidebarListEl.innerHTML = "";
    if (!count) {
      const empty = document.createElement("div");
      empty.className = "lf-sidebar-empty";
      empty.textContent = "No comments yet. Drag across text or click + on a code line to leave one.";
      sidebarListEl.appendChild(empty);
      return;
    }

    for (const note of ordered) {
      const item = document.createElement("div");
      item.className = "lf-sidebar-item";
      item.setAttribute("data-lf-sidebar-note", note.id);

      const q = document.createElement("div");
      q.className = "lf-anchor-quote";
      q.textContent = formatNoteAnchorLabel(note);
      item.appendChild(q);

      if (sidebarEditingId === note.id) {
        const ta = document.createElement("textarea");
        ta.value = note.text;
        ta.placeholder = "Edit comment… (⌘↩ to save)";
        const foot = document.createElement("div");
        foot.className = "lf-sidebar-item-foot";
        const cancelBtn = makeButton("Cancel", (e) => {
          e.stopPropagation();
          sidebarEditingId = null;
          renderSidebar();
        }, "lf-btn-quiet");
        const commit = () => {
          const val = oneLine(ta.value);
          if (!val) return;
          note.text = val;
          sidebarEditingId = null;
          refresh(true);
        };
        const saveBtn = makeButton("Save ⌘↩", (e) => {
          e.stopPropagation();
          commit();
        }, "lf-btn-accent");
        ta.addEventListener("keydown", (e) => {
          if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
            e.preventDefault();
            e.stopPropagation();
            commit();
          } else if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            sidebarEditingId = null;
            renderSidebar();
          }
        });
        foot.append(cancelBtn, saveBtn);
        item.append(ta, foot);
        setTimeout(() => {
          ta.focus();
          ta.setSelectionRange(ta.value.length, ta.value.length);
        }, 0);
      } else {
        const body = document.createElement("div");
        body.className = "lf-sidebar-item-text";
        body.textContent = note.text;
        const foot = document.createElement("div");
        foot.className = "lf-sidebar-item-foot";
        const editBtn = makeButton("Edit", (e) => {
          e.stopPropagation();
          sidebarEditingId = note.id;
          renderSidebar();
        }, "lf-btn-quiet");
        const delBtn = makeButton("Delete", (e) => {
          e.stopPropagation();
          deleteNote(note.id);
        }, "lf-btn-quiet");
        foot.append(editBtn, delBtn);
        item.append(body, foot);
      }

      item.addEventListener("click", (e) => {
        if (e.target.closest("button, textarea")) return;
        pulseElement(noteAnchorElement(note));
      });
      sidebarListEl.appendChild(item);
    }
  }

  // --- Browser T3 Code attached-chip composer dock (3b; hidden in Desktop) ---

  let dockEl = null;
  let dockChipsEl = null;
  let dockPreviewEl = null;
  let dockStatusEl = null;
  let dockOverallInput = null;
  let dockPreviewBtn = null;
  let dockClearBtn = null;
  let dockSendBtn = null;
  let dockPreviewOpen = false;
  let stripStatusText = "";
  let stripStatusKind = "";

  function steerTargetLabel() {
    if (work.kind === "task" && work.issue) return work.issue;
    if (work.kind === "wave" && work.wave) return "wave " + work.wave;
    if (work.kind === "session" && work.session) return "session";
    return "repository";
  }

  function buildSteerOperation(message) {
    if (work.kind === "task" && work.issue) {
      return {
        kind: "task.steer",
        directory: directory,
        issue: work.issue,
        message: message,
      };
    }
    return {
      kind: "session.steer",
      directory: directory,
      session: work.kind === "session" ? work.session || null : null,
      wave: work.kind === "wave" ? work.wave || null : null,
      message: message,
    };
  }

  function copyText(text) {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      return navigator.clipboard.writeText(text);
    }
    return new Promise((resolve, reject) => {
      const ta = document.createElement("textarea");
      ta.value = text;
      ta.style.position = "fixed";
      ta.style.opacity = "0";
      document.body.append(ta);
      ta.select();
      try {
        document.execCommand("copy");
        resolve();
      } catch (err) {
        reject(err);
      } finally {
        ta.remove();
      }
    });
  }

  // Drop unsent drafts while keeping rendered receipts; mark receipts reported only after a reply is sent.
  function clearDrafts(options) {
    const markReported = Boolean(options && options.markReported);
    const persist = !options || options.persist !== false;
    const receipts = markReported
      ? state.receipts.map((r) => Object.assign({}, r, { reported: true }))
      : state.receipts.slice();
    state = emptyState();
    state.receipts = receipts;
    sidebarEditingId = null;
    hideHoverCard();
    refresh(persist);
  }

  function submitReply() {
    if (desktopHandler) return;
    const items = queueItems();
    if (!items.length) return;
    const message = formatReplyMessage(items);

    if (hostMode === "loopback") {
      stripStatusText = "Sending…";
      stripStatusKind = "";
      renderDock();
      const op = buildSteerOperation(message);
      sendOperation(op, randomId()).then(
        (result) => {
          const sentence =
            (result && result.delivery && result.delivery.sentence) ||
            "Steer saved.";
          clearDrafts({ markReported: true });
          stripStatusText = sentence;
          stripStatusKind = "ok";
          renderDock();
        },
        (err) => {
          stripStatusText = errorMessage(err);
          stripStatusKind = "error";
          renderDock();
        }
      );
      return;
    }

    copyText(message).then(
      () => {
        stripStatusText = serverDisconnected
          ? "Copied reply (server disconnected)"
          : "Copied reply";
        stripStatusKind = "ok";
        renderDock();
      },
      (err) => {
        stripStatusText = "Copy failed: " + errorMessage(err);
        stripStatusKind = "error";
        renderDock();
      }
    );
  }

  function ensureDock() {
    if (desktopHandler || dockEl) return;

    dockEl = document.createElement("div");
    dockEl.className = "lf-dock";

    dockChipsEl = document.createElement("div");
    dockChipsEl.className = "lf-dock-chips";

    dockPreviewEl = document.createElement("div");
    dockPreviewEl.className = "lf-dock-preview";
    dockPreviewEl.hidden = true;

    dockStatusEl = document.createElement("div");
    dockStatusEl.className = "lf-dock-status";
    dockStatusEl.hidden = true;

    const row = document.createElement("div");
    row.className = "lf-dock-row";

    dockOverallInput = document.createElement("input");
    dockOverallInput.type = "text";
    dockOverallInput.className = "lf-dock-input";
    dockOverallInput.placeholder = "Overall direction (optional)…";
    dockOverallInput.setAttribute("aria-label", "Overall direction");
    dockOverallInput.addEventListener("input", () => {
      state.overall = dockOverallInput.value;
      stripStatusText = "";
      stripStatusKind = "";
      saveState();
      updateDockMeta();
    });

    const acts = document.createElement("div");
    acts.className = "lf-dock-actions";

    dockPreviewBtn = makeButton("Preview", () => {
      dockPreviewOpen = !dockPreviewOpen;
      updateDockMeta();
    }, "lf-btn-quiet");

    dockClearBtn = makeButton("Clear", () => {
      clearDrafts({ markReported: false });
    }, "lf-btn-quiet");

    dockSendBtn = makeButton("", submitReply, "lf-btn-accent");

    acts.append(dockPreviewBtn, dockClearBtn, dockSendBtn);
    row.append(dockOverallInput, acts);
    dockEl.append(dockChipsEl, dockPreviewEl, dockStatusEl, row);
    document.body.appendChild(dockEl);
  }

  function updateDockMeta() {
    if (desktopHandler || !dockEl) return;
    const items = queueItems();
    const hasItems = items.length > 0;

    if (dockOverallInput.value !== state.overall) {
      dockOverallInput.value = state.overall;
    }

    dockPreviewBtn.hidden = !hasItems;
    dockPreviewBtn.classList.toggle("is-selected", dockPreviewOpen && hasItems);
    dockPreviewBtn.setAttribute("aria-pressed", dockPreviewOpen && hasItems ? "true" : "false");

    dockClearBtn.hidden = !hasItems;

    dockSendBtn.disabled = !hasItems;
    dockSendBtn.textContent =
      hostMode === "loopback"
        ? "Steer " + steerTargetLabel() + " ⌘↩"
        : "Copy reply ⌘↩";

    dockPreviewEl.hidden = !(dockPreviewOpen && hasItems);
    if (dockPreviewOpen && hasItems) {
      dockPreviewEl.textContent = formatReplyMessage(items);
    }

    dockStatusEl.hidden = !stripStatusText;
    if (stripStatusText) {
      dockStatusEl.textContent = stripStatusText;
      dockStatusEl.className =
        "lf-dock-status" +
        (stripStatusKind === "error"
          ? " is-error"
          : stripStatusKind === "ok"
          ? " is-ok"
          : "");
    }
  }

  function renderDock() {
    if (desktopHandler) {
      if (dockEl) dockEl.hidden = true;
      return;
    }
    ensureDock();
    dockEl.hidden = false;

    const entries = queueEntries();
    dockChipsEl.innerHTML = "";
    dockChipsEl.hidden = entries.length === 0;
    for (const entry of entries) {
      const chip = document.createElement("span");
      chip.className = "lf-chip";
      chip.title = entry.title;

      const tag = document.createElement("span");
      tag.className = "lf-chip-tag tag-" + entry.tag;
      tag.textContent = entry.tag;

      const txt = document.createElement("span");
      txt.className = "lf-chip-text";
      txt.textContent = entry.summary;

      const x = document.createElement("button");
      x.type = "button";
      x.className = "lf-chip-x";
      x.textContent = "×";
      x.title = "Remove";
      x.setAttribute("aria-label", "Remove " + entry.tag);
      x.addEventListener("click", (e) => {
        e.stopPropagation();
        entry.remove();
      });

      chip.append(tag, txt, x);
      chip.addEventListener("click", () => entry.jump());
      dockChipsEl.appendChild(chip);
    }

    updateDockMeta();
  }

  // --- Global keyboard shortcuts ---

  document.addEventListener("keydown", (event) => {
    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      if (floatComposer && floatComposer.contains(document.activeElement)) return;
      if (sidebarEl && sidebarEl.contains(document.activeElement) && document.activeElement.tagName === "TEXTAREA") return;
      if (!desktopHandler && queueItems().length > 0) {
        event.preventDefault();
        submitReply();
      }
      return;
    }

    if (event.key === "Escape") {
      if (floatComposer || floatPill || hoverCardEl) {
        dismissFloats();
        hideHoverCard();
      } else if (sidebarOpen) {
        sidebarOpen = false;
        sidebarEditingId = null;
        renderSidebar();
      }
      return;
    }

    if (
      !event.metaKey &&
      !event.ctrlKey &&
      !event.altKey &&
      event.key.toLowerCase() === "c" &&
      pendingSelection &&
      !(document.activeElement && document.activeElement.closest("input, textarea"))
    ) {
      event.preventDefault();
      const selInfo = pendingSelection;
      dismissFloatPill();
      openFloatingComposer(selInfo.rect, {
        anchorLabel: "§ " + selInfo.section + ' · "' + truncate(selInfo.quote, 46) + '"',
        onSave: (val) => saveProseNote(selInfo, val),
      });
    }
  });

  wireControls();
  wireCodeBlocks();
  wireSelectionNotes();
  renderSidebar();

  if (desktopHandler) {
    try {
      const reply = desktopHandler.postMessage({
        type: "ready",
        path: pagePath,
        build_id: buildId,
      });
      if (reply && typeof reply.then === "function") {
        reply.then((loaded) => applyState(loaded), () => {});
      }
    } catch (_) {}
  } else {
    renderDock();
    loadBrowserState();
    connectLoopback();
  }
})();
