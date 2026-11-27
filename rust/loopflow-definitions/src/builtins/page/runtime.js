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
  const mainEl =
    document.querySelector("main.lf-page") || document.querySelector("main") || document.body;

  const desktopHandler =
    window.webkit &&
    window.webkit.messageHandlers &&
    window.webkit.messageHandlers.loopflow;

  let hostMode = desktopHandler ? "desktop" : "clipboard";
  let socket = null;
  let nextReqId = 1;
  const pending = new Map();
  const cancelledAction = Symbol("cancelled action");
  let serverDisconnected = false;
  let sendingReply = false;

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

  function el(tag, opts, ...children) {
    const node = document.createElement(tag);
    if (typeof opts === "string") {
      if (opts) node.className = opts;
    } else if (opts) {
      for (const [k, v] of Object.entries(opts)) {
        if (v == null || v === false) continue;
        if (k === "cls") node.className = v;
        else if (k === "text") node.textContent = v;
        else if (k === "on") {
          for (const [ev, fn] of Object.entries(v)) node.addEventListener(ev, fn);
        } else if (k === "attrs") {
          for (const [ak, av] of Object.entries(v)) node.setAttribute(ak, av);
        } else {
          node[k] = v;
        }
      }
    }
    for (const child of children) {
      if (child != null && child !== false) node.append(child);
    }
    return node;
  }

  function makeButton(label, onClick, className) {
    return el("button", { type: "button", cls: className || "", text: label, on: { click: onClick } });
  }

  function setPressed(btn, active) {
    btn.classList.toggle("is-selected", active);
    btn.setAttribute("aria-pressed", active ? "true" : "false");
  }

  function focusEnd(ta) {
    ta.focus();
    ta.setSelectionRange(ta.value.length, ta.value.length);
  }

  function clearSelection() {
    try {
      window.getSelection().removeAllRanges();
    } catch (_) {}
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
    let cur = node && node.nodeType === 1 ? node : node && node.parentElement;
    while (cur && cur !== document.body) {
      let prev = cur.previousElementSibling;
      while (prev) {
        if (/^H[1-4]$/.test(prev.tagName)) return oneLine(prev.textContent);
        prev = prev.previousElementSibling;
      }
      cur = cur.parentElement;
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

  function pulseElement(target) {
    if (!target) return;
    try {
      target.scrollIntoView({ behavior: "smooth", block: "center" });
    } catch (_) {}
    target.classList.add("lf-pulse-ring");
    setTimeout(() => target.classList.remove("lf-pulse-ring"), 950);
  }

  function rowsForLineNote(note) {
    const matched = [];
    const attr = note.side === "old" ? "data-old-line" : "data-line";
    const start = Number(note.lineStart);
    const end = Number(note.lineEnd || note.lineStart);
    for (const block of mainEl.querySelectorAll(".lf-codeblock[data-path]")) {
      if (block.getAttribute("data-path") !== note.path) continue;
      block.querySelectorAll("tr.lf-code-row[" + attr + "]").forEach((tr) => {
        const ln = Number(tr.getAttribute(attr));
        if (ln >= start && ln <= end) matched.push(tr);
      });
    }
    return matched;
  }

  function noteAnchorElement(note) {
    if (note.kind === "line") return rowsForLineNote(note)[0] || null;
    return mainEl.querySelector('.lf-hl[data-lf-note-id="' + CSS.escape(note.id) + '"]');
  }

  function notesInDocumentOrder() {
    if (state.notes.length <= 1) return state.notes.slice();
    return state.notes
      .map((note, idx) => ({ note, idx, el: noteAnchorElement(note) }))
      .sort((a, b) => {
        if (a.el && b.el && a.el !== b.el) {
          const pos = a.el.compareDocumentPosition(b.el);
          if (pos & Node.DOCUMENT_POSITION_FOLLOWING) return -1;
          if (pos & Node.DOCUMENT_POSITION_PRECEDING) return 1;
        }
        if (a.el !== b.el) return a.el ? -1 : 1;
        return a.idx - b.idx;
      })
      .map((entry) => entry.note);
  }

  function deleteNote(id) {
    state.notes = state.notes.filter((n) => n.id !== id);
    if (sidebarEditingId === id) sidebarEditingId = null;
    hideHoverCard();
    refresh(true);
  }

  function editNote(id, text) {
    state.notes = state.notes.map((note) => note.id === id ? { ...note, text } : note);
    sidebarEditingId = null;
    refresh(true);
  }

  function queueEntries() {
    const entries = [];
    for (const note of notesInDocumentOrder()) {
      const text = oneLine(note.text);
      const jump = () => pulseElement(noteAnchorElement(note));
      const remove = () => deleteNote(note.id);
      if (note.kind === "line") {
        const anchor = formatLineAnchor(note);
        entries.push({
          tag: "lines",
          summary: shortPath(note.path) + ":" + formatLineSpan(note) + " — " + text,
          title: anchor + " — " + text,
          ref: "`" + pagePath + "` § " + anchor + " — " + text,
          jump,
          remove,
        });
      } else {
        const section = oneLine(note.section);
        const quote = oneLine(note.quote);
        entries.push({
          tag: "note",
          summary: '"' + truncate(quote, 26) + '" — ' + text,
          title: section + ' › "' + quote + '" — ' + text,
          ref: "`" + pagePath + "` § " + section + ' › "' + quote + '" ' + text,
          jump,
          remove,
        });
      }
    }

    function pushControl(tag, attr, key, label, val, remove) {
      const summary = label + ": " + val;
      entries.push({
        tag,
        summary,
        title: summary,
        ref: "`" + pagePath + "` § " + summary,
        jump: () => pulseElement(document.querySelector("[" + attr + '="' + CSS.escape(key) + '"]')),
        remove: () => {
          remove();
          refresh(true);
        },
      });
    }

    for (const [group, entry] of Object.entries(state.choices)) {
      if (!entry || !entry.value) continue;
      pushControl(
        "choice",
        "data-choice",
        group,
        oneLine(entry.label || group),
        oneLine(entry.value),
        () => delete state.choices[group]
      );
    }
    for (const [group, entry] of Object.entries(state.multiselects)) {
      if (!entry || !Array.isArray(entry.values) || entry.values.length === 0) continue;
      pushControl(
        "choice",
        "data-multiselect",
        group,
        oneLine(entry.label || group),
        entry.values.map(oneLine).join(", "),
        () => delete state.multiselects[group]
      );
    }
    for (const [qid, entry] of Object.entries(state.questions)) {
      const ans = entry && oneLine(entry.answer);
      if (!ans) continue;
      pushControl(
        "answer",
        "data-question",
        qid,
        oneLine(entry.label || qid),
        ans,
        () => delete state.questions[qid]
      );
    }
    for (const act of state.offlineActions) {
      const summary = oneLine(act.summary);
      entries.push({
        tag: "run",
        summary,
        title: "Run: " + summary,
        ref: "`" + pagePath + "` Run: " + summary,
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

  function queueItems(entries) {
    const items = (entries || queueEntries()).map((e) => e.ref);
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
      try {
        desktopHandler.postMessage({
          type: "sync",
          build_id: buildId,
          state,
          items: queueItems(),
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
      if (parsed && typeof parsed === "object") applyState(parsed);
    } catch (_) {}
  }

  // `persist` marks an edit: the last send's sentence no longer describes the queue.
  function refresh(persist) {
    if (persist) setStatus("", "");
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
    if (window.location.protocol !== "http:" || window.location.hostname !== "127.0.0.1") return null;
    const parts = window.location.pathname.split("/").filter(Boolean);
    if (parts.length < 3 || parts[1] !== "f" || !/^[A-Za-z0-9_-]{43}$/.test(parts[0])) return null;
    return "ws://" + window.location.host + "/" + parts[0] + "/machine";
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
          client: { kind: "page", platform: "browser", name: "review-page", version: "1" },
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
      for (const [, entry] of pending) entry.reject(new Error("Connection closed"));
      pending.clear();
      if (!closingForUnload) markDisconnected(handshakeError);
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
            operation,
            request_id: requestId || null,
          })
        ).then((result) => {
          if (result && result.error) throw new Error(result.error);
          if (result && result.cancelled) return cancelledAction;
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
      try {
        socket.send(JSON.stringify({
          type: "request",
          id,
          operation,
          select: null,
          sender: null,
          request_id: requestId || null,
        }));
      } catch (error) {
        pending.delete(id);
        reject(error);
      }
    });
  }

  // --- Banner ---

  let bannerEl = null;
  function showBanner(text) {
    if (!bannerEl) {
      bannerEl = el("div", "lf-banner");
      mainEl.insertBefore(bannerEl, mainEl.firstChild);
    }
    bannerEl.textContent = text;
    bannerEl.hidden = false;
  }

  function hideBanner() {
    if (bannerEl) bannerEl.hidden = true;
  }

  // --- Controls & direct `data-lf` actions ---

  // Each `data-lf` button and the element now standing in its place.
  const actions = [];

  function showInSlot(action, node) {
    if (action.slot !== node) action.slot.replaceWith(node);
    action.slot = node;
  }

  function wireControls() {
    document.querySelectorAll("[data-notes-toggle]").forEach((btn) => {
      btn.addEventListener("click", () => {
        const block = btn.closest(".lf-codeblock");
        if (!block) return;
        const hidden = block.classList.toggle("lf-notes-hidden");
        btn.textContent = hidden ? "Show line notes" : "Hide line notes";
      });
    });

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

    document.querySelectorAll("[data-multiselect]").forEach((groupEl) => {
      const group = groupEl.getAttribute("data-multiselect");
      const label = groupEl.getAttribute("data-label") || group;
      groupEl.querySelectorAll("button").forEach((btn) => {
        btn.addEventListener("click", () => {
          const value = buttonValue(btn);
          const current = (state.multiselects[group] && state.multiselects[group].values) || [];
          const next = current.includes(value)
            ? current.filter((v) => v !== value)
            : current.concat([value]);
          if (next.length === 0) delete state.multiselects[group];
          else state.multiselects[group] = { label, values: next };
          refresh(true);
        });
      });
    });

    document.querySelectorAll("[data-question]").forEach((box) => {
      const qid = box.getAttribute("data-question");
      const label = box.getAttribute("data-label") || qid;
      let input = box.querySelector("textarea, input");
      if (!input) {
        input = el("textarea", { rows: 2 });
        box.append(input);
      }
      input.addEventListener("input", () => {
        const answer = input.value;
        if (!oneLine(answer)) delete state.questions[qid];
        else state.questions[qid] = { label, answer };
        setStatus("", "");
        saveState();
        renderDock();
      });
    });

    document.querySelectorAll("[data-lf]").forEach((btn, idx) => {
      let template;
      try {
        template = JSON.parse(btn.getAttribute("data-lf") || "{}");
      } catch (_) {
        return;
      }
      const action = {
        key: btn.getAttribute("data-lf-id") || "action-" + idx,
        btn,
        template,
        slot: btn,
      };
      actions.push(action);
      btn.addEventListener("click", (event) => {
        if (event.isTrusted) openActionConfirm(action);
      });
    });
  }

  // Receipts and offline-queued actions outlive reloads; an action with neither shows its button.
  function syncActionsFromState() {
    for (const action of actions) {
      const receipt = state.receipts.find((r) => r.key === action.key);
      const offline = state.offlineActions.find((a) => a.key === action.key);
      if (receipt) {
        showInSlot(action, el("span", { cls: "lf-action-receipt", text: receipt.text || receipt.summary }));
      } else if (offline) {
        showInSlot(action, el("span", { cls: "lf-action-summary", text: "Offline — added to reply" }));
      } else if (
        action.slot.classList.contains("lf-action-receipt") ||
        action.slot.classList.contains("lf-action-summary")
      ) {
        showInSlot(action, action.btn);
      }
    }
  }

  function syncControlsFromState() {
    syncActionsFromState();
    document.querySelectorAll("[data-choice]").forEach((groupEl) => {
      const selected =
        state.choices[groupEl.getAttribute("data-choice")] &&
        state.choices[groupEl.getAttribute("data-choice")].value;
      groupEl.querySelectorAll("button").forEach((btn) => setPressed(btn, selected === buttonValue(btn)));
    });
    document.querySelectorAll("[data-multiselect]").forEach((groupEl) => {
      const entry = state.multiselects[groupEl.getAttribute("data-multiselect")];
      const values = (entry && entry.values) || [];
      groupEl.querySelectorAll("button").forEach((btn) => setPressed(btn, values.includes(buttonValue(btn))));
    });
    document.querySelectorAll("[data-question]").forEach((box) => {
      let input = box.querySelector("textarea, input");
      if (!input) {
        input = el("textarea", { rows: 2 });
        box.append(input);
      }
      const entry = state.questions[box.getAttribute("data-question")];
      const saved = entry ? entry.answer : "";
      if (input.value !== saved) input.value = saved;
    });
  }

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
    if (copy.request_id === "__LF_REQUEST_ID__") copy.request_id = requestId;
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
    if (desktopHandler) execute();
    else {
      showInSlot(action, el(
        "span",
        "lf-action-confirm",
        el("span", { cls: "lf-action-summary", text: summarizeOperation(template) }),
        makeButton("Run", execute, "lf-btn-accent"),
        makeButton("Cancel", () => showInSlot(action, action.btn))
      ));
    }

    function execute(event) {
      if (event && !event.isTrusted) return;
      showInSlot(action, el("span", { cls: "lf-action-confirm", text: "Running…" }));
      const op = materializeOperation(template, requestId);
      sendOperation(op, requestId).then(
        (result) => {
          if (result === cancelledAction) {
            showInSlot(action, action.btn);
            return;
          }
          const text = formatReceipt(op.kind, result);
          state.receipts.push({ key: action.key, kind: op.kind, summary: resultTaskId(result) || text, text });
          refresh(true);
        },
        (err) => {
          showInSlot(
            action,
            el("span", "lf-action-error", el("span", { text: errorMessage(err) }), makeButton("Retry", execute))
          );
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
    let cur = node && node.nodeType === 1 ? node : node && node.parentElement;
    while (cur && cur !== mainEl) {
      if (BLOCK_TAGS.has(cur.tagName)) return cur;
      cur = cur.parentElement;
    }
    return mainEl;
  }

  function isExcludedTextNode(node) {
    const parent = node.parentElement;
    return (
      !parent ||
      Boolean(parent.closest(EXCLUDED_TEXT_SELECTOR)) ||
      Boolean(parent.closest("[hidden]")) ||
      Boolean(parent.closest(".lf-notes-hidden .lf-line-note"))
    );
  }

  function buildNormalizedIndex(root) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    let flat = "";
    const positions = [];
    let lastWasSpace = true;
    let lastBlock = null;
    let node;
    while ((node = walker.nextNode())) {
      if (!node.nodeValue || isExcludedTextNode(node)) continue;
      const block = nearestBlock(node);
      if (lastBlock && block !== lastBlock && !lastWasSpace) {
        flat += " ";
        positions.push(null);
        lastWasSpace = true;
      }
      lastBlock = block;
      const val = node.nodeValue;
      for (let i = 0; i < val.length; i++) {
        const ch = val[i];
        if (/\s/.test(ch)) {
          if (!lastWasSpace) {
            flat += " ";
            positions.push({ node, offset: i });
            lastWasSpace = true;
          }
        } else {
          flat += ch;
          positions.push({ node, offset: i });
          lastWasSpace = false;
        }
      }
    }
    if (flat.endsWith(" ")) {
      flat = flat.slice(0, -1);
      positions.pop();
    }
    return { flat, positions };
  }

  function findOccurrenceForRange(range, needle) {
    if (!needle) return 0;
    const index = buildNormalizedIndex(mainEl);
    let rangeFlatPos = -1;
    for (let i = 0; i < index.positions.length; i++) {
      const p = index.positions[i];
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
    while (s <= e && !index.positions[s]) s++;
    while (e >= s && !index.positions[e]) e--;
    if (s > e || !index.positions[s] || !index.positions[e]) return null;
    try {
      const range = document.createRange();
      range.setStart(index.positions[s].node, index.positions[s].offset);
      range.setEnd(index.positions[e].node, index.positions[e].offset + 1);
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
      try {
        if (!range.intersectsNode(node)) continue;
      } catch (_) {
        continue;
      }
      const start = node === range.startContainer ? range.startOffset : 0;
      const end = node === range.endContainer ? range.endOffset : node.nodeValue.length;
      if (end > start && node.nodeValue.slice(start, end).trim()) {
        hits.push({ node, start, end });
      }
    }
    for (let i = hits.length - 1; i >= 0; i--) {
      const { node: tn, start, end } = hits[i];
      const text = tn.nodeValue;
      const frag = document.createDocumentFragment();
      if (start > 0) frag.append(text.slice(0, start));
      frag.append(el("span", { cls: "lf-hl", text: text.slice(start, end), attrs: { "data-lf-note-id": noteId } }));
      if (end < text.length) frag.append(text.slice(end));
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

  let lastHighlightSig = "";

  function renderHighlights() {
    const sig = JSON.stringify(
      state.notes.map((n) =>
        n.kind === "line"
          ? [n.id, "line", n.path, n.side, n.lineStart, n.lineEnd]
          : [n.id, "prose", n.match || n.quote, n.occurrence || 0]
      )
    );
    if (sig === lastHighlightSig) return;
    lastHighlightSig = sig;

    unwrapAllHighlights();
    mainEl.querySelectorAll("tr.lf-code-row.lf-row-has-note").forEach((tr) => {
      tr.classList.remove("lf-row-has-note");
      tr.removeAttribute("data-lf-line-note-ids");
    });

    for (const note of state.notes) {
      if (note.kind === "line") {
        for (const tr of rowsForLineNote(note)) {
          tr.classList.add("lf-row-has-note");
          const existing = tr.getAttribute("data-lf-line-note-ids");
          const list = existing ? existing.split(",") : [];
          if (!list.includes(note.id)) list.push(note.id);
          tr.setAttribute("data-lf-line-note-ids", list.join(","));
        }
      } else {
        const range = findNormalizedRange(oneLine(note.match || note.quote), note.occurrence || 0);
        if (range) wrapRange(range, note.id);
      }
    }
  }

  // --- Floating selection pill, composer, and hover cards (1c) ---

  let floatPill = null;
  let floatComposer = null;
  let pendingSelection = null; // { range, rect, section, quote, match }
  let hoverCardEl = null;
  let hoverHideTimer = null;

  function dismissFloatPill() {
    if (floatPill) {
      floatPill.remove();
      floatPill = null;
    }
  }

  function clearPendingSelection() {
    pendingSelection = null;
    dismissFloatPill();
  }

  function dismissFloatComposer() {
    if (floatComposer) {
      floatComposer.remove();
      floatComposer = null;
    }
    mainEl.querySelectorAll("tr.lf-code-row.lf-row-selecting").forEach((tr) => {
      tr.classList.remove("lf-row-selecting");
    });
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

  function clampFixedBox(box, rect, preferAbove) {
    const w = box.offsetWidth;
    const h = box.offsetHeight;
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
    box.style.top = top + "px";
    box.style.left = left + "px";
  }

  function buildNoteEditor({ initialText, placeholder, rows, onSave, onCancel, stopProp }) {
    const ta = el("textarea", { rows: rows || 2, placeholder, value: initialText || "" });
    const commit = () => {
      const val = oneLine(ta.value);
      if (val) onSave(val);
    };
    ta.addEventListener("keydown", (e) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
        e.preventDefault();
        e.stopPropagation();
        commit();
      } else if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        onCancel();
      }
    });
    const wrap = (fn) => (e) => {
      if (stopProp) e.stopPropagation();
      fn();
    };
    const actionsEl = el(
      "div",
      "lf-card-actions",
      makeButton("Cancel", wrap(onCancel), "lf-btn-quiet"),
      makeButton("Save ⌘↩", wrap(commit), "lf-btn-accent")
    );
    return { ta, actionsEl };
  }

  function showFloatPill(rect, buttons) {
    dismissFloatPill();
    floatPill = el("div", "lf-float-pill");
    buttons.forEach((b, idx) => {
      if (idx > 0) floatPill.append(el("span", "lf-float-pill-sep"));
      floatPill.append(
        el("button", {
          type: "button",
          text: b.label,
          on: {
            mousedown: (e) => e.preventDefault(),
            click: () => {
              dismissFloatPill();
              b.onClick();
            },
          },
        })
      );
    });
    document.body.append(floatPill);
    clampFixedBox(floatPill, rect, true);
  }

  function saveProseNote(selInfo, text) {
    const needle = selInfo.match;
    state.notes.push({
      id: randomId(),
      kind: "prose",
      section: selInfo.section,
      quote: selInfo.quote.slice(0, 140),
      match: needle,
      occurrence: findOccurrenceForRange(selInfo.range, needle),
      text: oneLine(text),
    });
    pendingSelection = null;
    clearSelection();
    refresh(true);
  }

  function openProseComposer(selInfo) {
    openFloatingComposer(selInfo.rect, {
      anchorLabel: formatNoteAnchorLabel({ kind: "prose", section: selInfo.section, quote: selInfo.quote }),
      onSave: (val) => saveProseNote(selInfo, val),
    });
  }

  function openFloatingComposer(rect, opts) {
    dismissFloats();
    hideHoverCard();

    if (opts.selectedRows) {
      for (const tr of opts.selectedRows) tr.classList.add("lf-row-selecting");
    }

    const { ta, actionsEl } = buildNoteEditor({
      initialText: opts.initialText,
      placeholder: opts.placeholder || "Add comment… (⌘↩ to save)",
      onSave: (val) => {
        dismissFloatComposer();
        opts.onSave(val);
      },
      onCancel: dismissFloatComposer,
    });

    floatComposer = el(
      "div",
      "lf-float-composer",
      el("div", { cls: "lf-anchor-quote", text: opts.anchorLabel }),
      ta,
      actionsEl
    );
    document.body.append(floatComposer);
    clampFixedBox(floatComposer, rect, false);
    focusEnd(ta);
  }

  function showHoverCard(targetEl, note) {
    if (floatComposer) return;
    hideHoverCard();

    const editBtn = makeButton("Edit", () => {
      const r = targetEl.getBoundingClientRect();
      hideHoverCard();
      openFloatingComposer(r, {
        anchorLabel: formatNoteAnchorLabel(note),
        initialText: note.text,
        onSave: (val) => editNote(note.id, val),
      });
    }, "lf-btn-quiet");

    hoverCardEl = el(
      "div",
      {
        cls: "lf-hover-card",
        on: {
          mouseenter: () => clearTimeout(hoverHideTimer),
          mouseleave: () => {
            hoverHideTimer = setTimeout(hideHoverCard, 200);
          },
        },
      },
      el("div", { cls: "lf-anchor-quote", text: formatNoteAnchorLabel(note) }),
      el("div", { cls: "lf-card-body", text: note.text }),
      el("div", "lf-card-actions", editBtn, makeButton("Delete", () => deleteNote(note.id), "lf-btn-quiet"))
    );
    document.body.append(hoverCardEl);
    clampFixedBox(hoverCardEl, targetEl.getBoundingClientRect(), false);
  }

  function wireSelectionNotes() {
    document.addEventListener("mouseup", (event) => {
      if (
        event.target.closest(
          ".lf-float-pill, .lf-float-composer, .lf-hover-card, .lf-sidebar, .lf-dock, .lf-comments-toggle, button, textarea, input, .lf-action-confirm, td.lf-ln"
        )
      ) {
        return;
      }

      const sel = window.getSelection();
      if (!sel || sel.isCollapsed) return clearPendingSelection();
      const quote = oneLine(sel.toString());
      if (!quote) return clearPendingSelection();

      const range = sel.getRangeAt(0).cloneRange();
      const containerEl =
        range.commonAncestorContainer.nodeType === 1
          ? range.commonAncestorContainer
          : range.commonAncestorContainer.parentElement;
      if (!containerEl || !mainEl.contains(containerEl)) return clearPendingSelection();

      const selInfo = {
        range,
        rect: range.getBoundingClientRect(),
        section: nearestSection(containerEl),
        quote: quote.slice(0, 140),
        match: quote.slice(0, 240),
      };
      pendingSelection = selInfo;

      showFloatPill(selInfo.rect, [
        { label: "Comment (c)", onClick: () => openProseComposer(selInfo) },
        { label: "Flag", onClick: () => saveProseNote(selInfo, "Flagged for follow-up") },
        {
          label: "Copy",
          onClick: () => {
            copyText(quote).catch(() => {});
            pendingSelection = null;
            clearSelection();
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

    mainEl.addEventListener("mouseover", (event) => {
      const hl = event.target.closest(".lf-hl[data-lf-note-id]");
      if (hl) {
        const note = state.notes.find((n) => n.id === hl.getAttribute("data-lf-note-id"));
        if (note) showHoverCard(hl, note);
        return;
      }
      const lnCell = event.target.closest("tr.lf-code-row.lf-row-has-note td.lf-ln");
      if (lnCell) {
        const tr = lnCell.closest("tr.lf-code-row");
        const firstId = (tr.getAttribute("data-lf-line-note-ids") || "").split(",")[0];
        const note = firstId ? state.notes.find((n) => n.id === firstId) : null;
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

  let lastGutterAnchor = null; // { block, newLine, oldLine }

  function rowLineInfo(tr) {
    const n = tr.getAttribute("data-line");
    const o = tr.getAttribute("data-old-line");
    const newLine = n !== null && n !== "" ? Number(n) : null;
    const oldLine = o !== null && o !== "" ? Number(o) : null;
    return newLine !== null || oldLine !== null ? { newLine, oldLine } : null;
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
        const label = info.newLine !== null ? "line " + info.newLine : "old line " + info.oldLine;
        firstLn.insertBefore(
          el("button", {
            type: "button",
            cls: "lf-gutter-plus",
            text: "+",
            title: "Comment on " + label + " (Shift+click for range)",
            attrs: { "aria-label": "Comment on " + path + " " + label },
          }),
          firstLn.firstChild
        );
      });

      block.addEventListener("mousedown", (event) => {
        if (event.shiftKey && event.target.closest("td.lf-ln")) event.preventDefault();
      });

      block.addEventListener("click", (event) => {
        const trigger = event.target.closest(".lf-gutter-plus, td.lf-ln");
        if (!trigger) return;
        const tr = trigger.closest("tr.lf-code-row");
        const info = tr && rowLineInfo(tr);
        if (!info) return;
        event.preventDefault();
        dismissFloatPill();
        hideHoverCard();

        let rangeSide = info.newLine !== null ? "new" : "old";
        let lineStart = info.newLine !== null ? info.newLine : info.oldLine;
        let lineEnd = lineStart;

        if (event.shiftKey && lastGutterAnchor && lastGutterAnchor.block === block) {
          if (lastGutterAnchor.newLine !== null && info.newLine !== null) {
            rangeSide = "new";
            lineStart = Math.min(lastGutterAnchor.newLine, info.newLine);
            lineEnd = Math.max(lastGutterAnchor.newLine, info.newLine);
          } else if (lastGutterAnchor.oldLine !== null && info.oldLine !== null) {
            rangeSide = "old";
            lineStart = Math.min(lastGutterAnchor.oldLine, info.oldLine);
            lineEnd = Math.max(lastGutterAnchor.oldLine, info.oldLine);
          }
        }
        lastGutterAnchor = { block, newLine: info.newLine, oldLine: info.oldLine };

        const previewNote = { kind: "line", path, side: rangeSide, lineStart, lineEnd };
        const selectedRows = rowsForLineNote(previewNote);
        // Span the whole range so a composer that flips above it never covers it.
        const firstRect = (selectedRows[0] || tr).getBoundingClientRect();
        const anchorRow = selectedRows[selectedRows.length - 1] || tr;
        const rowRect = anchorRow.getBoundingClientRect();
        const codeCell = anchorRow.lastElementChild;
        const codeLeft = codeCell ? codeCell.getBoundingClientRect().left : rowRect.left + 110;
        const composerRect = {
          top: firstRect.top,
          bottom: rowRect.bottom,
          left: codeLeft + 12,
          right: rowRect.right,
          width: Math.max(0, rowRect.right - codeLeft - 12),
        };

        openFloatingComposer(composerRect, {
          anchorLabel: formatLineAnchor(previewNote),
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
  let commentsCountEl = null;
  let sidebarEl = null;
  let sidebarTitleEl = null;
  let sidebarListEl = null;
  let sidebarOpen = false;
  let sidebarEditingId = null;

  function closeSidebar() {
    sidebarOpen = false;
    sidebarEditingId = null;
    renderSidebar();
  }

  function ensureSidebar() {
    if (sidebarEl) return;

    commentsCountEl = el("span", { cls: "lf-comments-count", hidden: true });
    commentsToggleBtn = el(
      "button",
      {
        type: "button",
        cls: "lf-comments-toggle",
        on: {
          click: () => {
            sidebarOpen = !sidebarOpen;
            renderSidebar();
          },
        },
      },
      el("span", { text: "Comments" }),
      commentsCountEl
    );

    sidebarTitleEl = el("span");
    sidebarListEl = el("div", "lf-sidebar-list");
    sidebarEl = el(
      "aside",
      { cls: "lf-sidebar", attrs: { "aria-label": "Comments sidebar" } },
      el("div", "lf-sidebar-head", sidebarTitleEl, makeButton("Close", closeSidebar, "lf-btn-quiet")),
      sidebarListEl
    );
    document.body.append(commentsToggleBtn, sidebarEl);
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

    commentsCountEl.textContent = String(count);
    commentsCountEl.hidden = count === 0;
    commentsToggleBtn.classList.toggle("has-notes", count > 0);
    commentsToggleBtn.setAttribute("aria-expanded", sidebarOpen ? "true" : "false");

    sidebarEl.classList.toggle("is-open", sidebarOpen);
    sidebarTitleEl.textContent = count > 0 ? "Comments (" + count + ")" : "Comments";

    sidebarListEl.innerHTML = "";
    if (!count) {
      sidebarListEl.append(
        el("div", {
          cls: "lf-sidebar-empty",
          text: "No comments yet. Drag across text or click + on a code line to leave one.",
        })
      );
      return;
    }

    for (const note of ordered) {
      const item = el(
        "div",
        {
          cls: "lf-sidebar-item",
          attrs: { "data-lf-sidebar-note": note.id },
          on: {
            click: (e) => {
              if (!e.target.closest("button, textarea")) pulseElement(noteAnchorElement(note));
            },
          },
        },
        el("div", { cls: "lf-anchor-quote", text: formatNoteAnchorLabel(note) })
      );

      if (sidebarEditingId === note.id) {
        const { ta, actionsEl } = buildNoteEditor({
          initialText: note.text,
          placeholder: "Edit comment… (⌘↩ to save)",
          stopProp: true,
          onSave: (val) => editNote(note.id, val),
          onCancel: () => {
            sidebarEditingId = null;
            renderSidebar();
          },
        });
        item.append(ta, actionsEl);
        setTimeout(() => focusEnd(ta), 0);
      } else {
        item.append(
          el("div", { cls: "lf-card-body", text: note.text }),
          el(
            "div",
            "lf-card-actions",
            makeButton(
              "Edit",
              (e) => {
                e.stopPropagation();
                sidebarEditingId = note.id;
                renderSidebar();
              },
              "lf-btn-quiet"
            ),
            makeButton(
              "Delete",
              (e) => {
                e.stopPropagation();
                deleteNote(note.id);
              },
              "lf-btn-quiet"
            )
          )
        );
      }
      sidebarListEl.append(item);
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
  let statusText = "";
  let statusKind = "";

  function setStatus(text, kind) {
    statusText = text;
    statusKind = kind || "";
  }

  function steerTargetLabel() {
    if (work.kind === "task" && work.issue) return work.issue;
    if (work.kind === "wave" && work.wave) return "wave " + work.wave;
    if (work.kind === "session" && work.session) return "session";
    return "repository";
  }

  function buildSteerOperation(message) {
    if (work.kind === "task" && work.issue) {
      return { kind: "task.steer", directory, issue: work.issue, message };
    }
    return {
      kind: "session.steer",
      directory,
      session: work.kind === "session" ? work.session || null : null,
      wave: work.kind === "wave" ? work.wave || null : null,
      message,
    };
  }

  function copyText(text) {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      return navigator.clipboard.writeText(text);
    }
    return new Promise((resolve, reject) => {
      const ta = el("textarea", { value: text });
      ta.style.position = "fixed";
      ta.style.opacity = "0";
      document.body.append(ta);
      ta.select();
      try {
        if (document.execCommand("copy")) resolve();
        else reject(new Error("Clipboard is unavailable"));
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
    if (desktopHandler || sendingReply) return;
    const items = queueItems();
    if (!items.length) return;
    const message = formatReplyMessage(items);

    if (hostMode === "loopback") {
      // Retain edits made after the send, including receipts from actions
      // that finish while the steer is in flight.
      const sent = {
        notes: new Set(state.notes),
        choices: { ...state.choices },
        multiselects: { ...state.multiselects },
        questions: { ...state.questions },
        offlineActions: new Set(state.offlineActions),
        overall: state.overall,
        receipts: new Set(state.receipts),
      };
      sendingReply = true;
      setStatus("Sending…", "");
      renderDock();
      sendOperation(buildSteerOperation(message), randomId()).then(
        (result) => {
          sendingReply = false;
          state.notes = state.notes.filter((note) => !sent.notes.has(note));
          state.offlineActions = state.offlineActions.filter((action) => !sent.offlineActions.has(action));
          for (const group of ["choices", "multiselects", "questions"]) {
            for (const [key, value] of Object.entries(sent[group])) {
              if (state[group][key] === value) delete state[group][key];
            }
          }
          if (state.overall === sent.overall) state.overall = "";
          for (const receipt of state.receipts) {
            if (sent.receipts.has(receipt)) receipt.reported = true;
          }
          setStatus(result?.delivery?.sentence || "Steer saved.", "ok");
          saveState();
          refresh(false);
        },
        (err) => {
          sendingReply = false;
          setStatus(errorMessage(err), "error");
          renderDock();
        }
      );
      return;
    }

    copyText(message).then(
      () => {
        setStatus(serverDisconnected ? "Copied reply (server disconnected)" : "Copied reply", "ok");
        renderDock();
      },
      (err) => {
        setStatus("Copy failed: " + errorMessage(err), "error");
        renderDock();
      }
    );
  }

  function ensureDock() {
    if (desktopHandler || dockEl) return;

    dockChipsEl = el("div", "lf-dock-chips");
    dockPreviewEl = el("div", { cls: "lf-dock-preview", hidden: true });
    dockStatusEl = el("div", { cls: "lf-dock-status", hidden: true });

    dockOverallInput = el("input", {
      type: "text",
      cls: "lf-dock-input",
      placeholder: "Overall direction (optional)…",
      attrs: { "aria-label": "Overall direction" },
      on: {
        input: () => {
          state.overall = dockOverallInput.value;
          setStatus("", "");
          saveState();
          updateDockMeta();
        },
      },
    });

    dockPreviewBtn = makeButton("Preview", () => {
      dockPreviewOpen = !dockPreviewOpen;
      updateDockMeta();
    }, "lf-btn-quiet");
    dockClearBtn = makeButton("Clear", () => clearDrafts({ markReported: false }), "lf-btn-quiet");
    dockSendBtn = makeButton("", submitReply, "lf-btn-accent");

    dockEl = el(
      "div",
      "lf-dock",
      dockChipsEl,
      dockPreviewEl,
      dockStatusEl,
      el("div", "lf-dock-row", dockOverallInput, el("div", "lf-dock-actions", dockPreviewBtn, dockClearBtn, dockSendBtn))
    );
    document.body.append(dockEl);
    // Reserve the dock's height below the page so the last control stays reachable.
    new ResizeObserver(() => {
      document.documentElement.style.setProperty("--lf-dock-room", dockEl.offsetHeight + 40 + "px");
    }).observe(dockEl);
  }

  function updateDockMeta(entries) {
    if (desktopHandler || !dockEl) return;
    const items = queueItems(entries);
    const hasItems = items.length > 0;

    if (dockOverallInput.value !== state.overall) dockOverallInput.value = state.overall;

    dockPreviewBtn.hidden = !hasItems;
    setPressed(dockPreviewBtn, dockPreviewOpen && hasItems);
    dockClearBtn.hidden = !hasItems;

    dockSendBtn.disabled = !hasItems || sendingReply;
    dockSendBtn.textContent =
      hostMode === "loopback" ? "Steer " + steerTargetLabel() + " ⌘↩" : "Copy reply ⌘↩";

    dockPreviewEl.hidden = !(dockPreviewOpen && hasItems);
    if (dockPreviewOpen && hasItems) dockPreviewEl.textContent = formatReplyMessage(items);

    dockStatusEl.hidden = !statusText;
    if (statusText) {
      dockStatusEl.textContent = statusText;
      dockStatusEl.className =
        "lf-dock-status" + (statusKind === "error" ? " is-error" : statusKind === "ok" ? " is-ok" : "");
    }
  }

  function renderDock() {
    if (desktopHandler) return;
    ensureDock();

    const entries = queueEntries();
    dockChipsEl.innerHTML = "";
    dockChipsEl.hidden = entries.length === 0;
    for (const entry of entries) {
      dockChipsEl.append(
        el(
          "span",
          { cls: "lf-chip", title: entry.title, on: { click: () => entry.jump() } },
          el("span", { cls: "lf-chip-tag tag-" + entry.tag, text: entry.tag[0].toUpperCase() + entry.tag.slice(1) }),
          el("span", { cls: "lf-chip-text", text: entry.summary }),
          el("button", {
            type: "button",
            cls: "lf-chip-x",
            text: "×",
            title: "Remove",
            attrs: { "aria-label": "Remove " + entry.tag },
            on: {
              click: (e) => {
                e.stopPropagation();
                entry.remove();
              },
            },
          })
        )
      );
    }

    updateDockMeta(entries);
  }

  // --- Global keyboard shortcuts ---

  document.addEventListener("keydown", (event) => {
    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      if (floatComposer && floatComposer.contains(document.activeElement)) return;
      if (sidebarEl && sidebarEl.contains(document.activeElement) && document.activeElement.tagName === "TEXTAREA") {
        return;
      }
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
        closeSidebar();
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
      openProseComposer(selInfo);
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
