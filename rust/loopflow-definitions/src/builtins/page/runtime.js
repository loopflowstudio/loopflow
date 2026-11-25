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
  // Notes, choices, multiselects, questions, and offline actions enter the reply queue.
  // Executed direct `data-lf` receipts stay separate and only ride as a suffix on ⌘↩.
  function emptyState() {
    return {
      notes: [],
      choices: {},
      multiselects: {},
      questions: {},
      offlineActions: [],
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

  function queueItems() {
    const items = [];
    for (const note of state.notes) {
      items.push(
        "`" +
          pagePath +
          "` § " +
          oneLine(note.section) +
          ' › "' +
          oneLine(note.quote) +
          '" ' +
          oneLine(note.text)
      );
    }
    for (const [group, entry] of Object.entries(state.choices)) {
      if (!entry || !entry.value) continue;
      items.push("`" + pagePath + "` § " + oneLine(entry.label || group) + ": " + oneLine(entry.value));
    }
    for (const [group, entry] of Object.entries(state.multiselects)) {
      if (!entry || !Array.isArray(entry.values) || entry.values.length === 0) continue;
      items.push(
        "`" + pagePath + "` § " + oneLine(entry.label || group) + ": " + entry.values.map(oneLine).join(", ")
      );
    }
    for (const [qid, entry] of Object.entries(state.questions)) {
      if (!entry || !oneLine(entry.answer)) continue;
      items.push("`" + pagePath + "` § " + oneLine(entry.label || qid) + ": " + oneLine(entry.answer));
    }
    for (const act of state.offlineActions) {
      items.push("`" + pagePath + "` Run: " + oneLine(act.summary));
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
    renderNotesTray();
    renderHighlights();
    if (persist) saveState();
    renderStrip();
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
    renderStrip();
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
        renderStrip();
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
      const main = document.querySelector("main") || document.body;
      main.insertBefore(bannerEl, main.firstChild);
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
        saveState();
        renderStrip();
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

  // --- Highlight-to-comment ---

  let popover = null;

  function dismissPopover() {
    if (popover) {
      popover.remove();
      popover = null;
    }
  }

  function wireSelectionNotes() {
    document.addEventListener("mouseup", (event) => {
      if (popover && popover.contains(event.target)) return;
      if (stripEl && stripEl.contains(event.target)) return;
      if (event.target.closest("button, textarea, input, .lf-action-confirm")) return;

      const sel = window.getSelection();
      if (!sel || sel.isCollapsed) {
        dismissPopover();
        return;
      }
      const quote = oneLine(sel.toString());
      if (!quote) {
        dismissPopover();
        return;
      }
      const range = sel.getRangeAt(0).cloneRange();
      const rect = range.getBoundingClientRect();
      const section = nearestSection(range.commonAncestorContainer);
      showNoteTrigger(rect, section, quote);
    });
  }

  function showNoteTrigger(rect, section, quote) {
    dismissPopover();
    popover = document.createElement("div");
    popover.className = "lf-note-popover";
    popover.style.top = Math.max(8, Math.min(window.innerHeight - 120, rect.bottom + 8)) + "px";
    popover.style.left = Math.max(8, Math.min(window.innerWidth - 280, rect.left)) + "px";

    const trigger = makeButton("Note", () => {
      popover.innerHTML = "";
      const ta = document.createElement("textarea");
      ta.placeholder = "Add note…";
      const actions = document.createElement("div");
      actions.className = "lf-note-popover-actions";
      function commit() {
        const text = oneLine(ta.value);
        if (!text) return;
        state.notes.push({ id: randomId(), section, quote: quote.slice(0, 140), text });
        dismissPopover();
        try {
          window.getSelection().removeAllRanges();
        } catch (_) {}
        refresh(true);
      }
      ta.addEventListener("keydown", (e) => {
        if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
          e.preventDefault();
          commit();
        } else if (e.key === "Escape") {
          dismissPopover();
        }
      });
      actions.append(makeButton("Cancel", dismissPopover), makeButton("Save note", commit, "lf-btn-accent"));
      popover.append(ta, actions);
      ta.focus();
    }, "lf-btn-accent");
    popover.append(trigger);
    document.body.append(popover);
  }

  let trayEl = null;
  function renderNotesTray() {
    if (!state.notes.length) {
      if (trayEl) trayEl.remove();
      trayEl = null;
      return;
    }
    if (!trayEl) {
      trayEl = document.createElement("section");
      trayEl.className = "lf-notes-tray";
      const main = document.querySelector("main") || document.body;
      main.append(trayEl);
    }
    trayEl.innerHTML = "";
    const heading = document.createElement("h3");
    heading.textContent = "Your notes";
    trayEl.append(heading);
    for (const note of state.notes) {
      const chip = document.createElement("div");
      chip.className = "lf-note-chip";
      const body = document.createElement("span");
      body.textContent = note.section + ' › "' + note.quote + '" — ' + note.text;
      const del = makeButton("Remove", () => {
        state.notes = state.notes.filter((n) => n.id !== note.id);
        refresh(true);
      }, "lf-btn-quiet");
      chip.append(body, del);
      trayEl.append(chip);
    }
  }

  function renderHighlights() {
    if (typeof Highlight === "undefined" || !CSS || !CSS.highlights) return;
    if (!state.notes.length) {
      CSS.highlights.delete("lf-note");
      return;
    }
    const ranges = [];
    const main = document.querySelector("main") || document.body;
    const walker = document.createTreeWalker(main, NodeFilter.SHOW_TEXT);
    const textNodes = [];
    let n;
    while ((n = walker.nextNode())) {
      if (n.parentElement && n.parentElement.closest(".lf-notes-tray, .lf-strip, .lf-note-popover")) {
        continue;
      }
      textNodes.push(n);
    }
    for (const note of state.notes) {
      const needle = note.quote;
      if (!needle) continue;
      for (const node of textNodes) {
        const idx = node.nodeValue.indexOf(needle);
        if (idx >= 0) {
          const r = document.createRange();
          r.setStart(node, idx);
          r.setEnd(node, idx + needle.length);
          ranges.push(r);
          break;
        }
      }
    }
    if (ranges.length) CSS.highlights.set("lf-note", new Highlight(...ranges));
    else CSS.highlights.delete("lf-note");
  }

  // --- Bottom reply strip ---

  let stripEl = null;
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
      renderStrip();
      const op = buildSteerOperation(message);
      sendOperation(op, randomId()).then(
        (result) => {
          const sentence =
            (result && result.delivery && result.delivery.sentence) ||
            "Steer saved.";
          clearDrafts({ markReported: true });
          stripStatusText = sentence;
          stripStatusKind = "ok";
          renderStrip();
        },
        (err) => {
          stripStatusText = errorMessage(err);
          stripStatusKind = "error";
          renderStrip();
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
        renderStrip();
      },
      (err) => {
        stripStatusText = "Copy failed: " + errorMessage(err);
        stripStatusKind = "error";
        renderStrip();
      }
    );
  }

  function renderStrip() {
    if (desktopHandler) {
      if (stripEl) stripEl.hidden = true;
      return;
    }
    const items = queueItems();
    if (!items.length && !stripStatusText) {
      if (stripEl) stripEl.hidden = true;
      return;
    }
    if (!stripEl) {
      stripEl = document.createElement("div");
      stripEl.className = "lf-strip";
      document.body.append(stripEl);
    }
    stripEl.hidden = false;
    stripEl.innerHTML = "";

    const count = document.createElement("span");
    count.textContent =
      items.length === 1 ? "1 item" : items.length + " items";
    if (items.length > 0) stripEl.append(count);

    if (stripStatusText) {
      const status = document.createElement("span");
      status.className =
        "lf-strip-status" +
        (stripStatusKind === "error"
          ? " is-error"
          : stripStatusKind === "ok"
          ? " is-ok"
          : "");
      status.textContent = stripStatusText;
      stripEl.append(status);
    }

    if (items.length > 0) {
      const clearBtn = makeButton("Clear", () => {
        clearDrafts({ markReported: false });
      }, "lf-btn-quiet");
      const sendLabel =
        hostMode === "loopback"
          ? "Steer " + steerTargetLabel() + " ⌘↩"
          : "Copy reply ⌘↩";
      stripEl.append(clearBtn, makeButton(sendLabel, submitReply, "lf-btn-accent"));
    }
  }

  document.addEventListener("keydown", (event) => {
    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      if (popover && popover.contains(document.activeElement)) return;
      if (!desktopHandler && queueItems().length > 0) {
        event.preventDefault();
        submitReply();
      }
    }
  });

  wireControls();
  wireSelectionNotes();

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
    loadBrowserState();
    connectLoopback();
  }
})();
