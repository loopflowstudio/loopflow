document.body.insertAdjacentHTML('afterbegin', "<svg width=\"0\" height=\"0\" style=\"position:absolute\" aria-hidden=\"true\">\n  <symbol id=\"i-side\" viewBox=\"0 0 16 16\"><rect x=\"2\" y=\"3\" width=\"12\" height=\"10\" rx=\"2\"/><path d=\"M6 3v10\"/></symbol>\n  <symbol id=\"i-left\" viewBox=\"0 0 16 16\"><path d=\"M10 3 5 8l5 5\"/></symbol>\n  <symbol id=\"i-right\" viewBox=\"0 0 16 16\"><path d=\"m6 3 5 5-5 5\"/></symbol>\n  <symbol id=\"i-down\" viewBox=\"0 0 16 16\"><path d=\"m4 6 4 4 4-4\"/></symbol>\n  <symbol id=\"i-plus\" viewBox=\"0 0 16 16\"><path d=\"M8 3v10M3 8h10\"/></symbol>\n  <symbol id=\"i-wave\" viewBox=\"0 0 16 16\"><path d=\"M2 5c2-2 4 2 6 0s4 2 6 0M2 8.5c2-2 4 2 6 0s4 2 6 0M2 12c2-2 4 2 6 0s4 2 6 0\"/></symbol>\n  <symbol id=\"i-restart\" viewBox=\"0 0 16 16\"><path d=\"M13 8a5 5 0 1 1-1.5-3.6M13 2.5V5h-2.5\"/></symbol>\n  <symbol id=\"i-chat\" viewBox=\"0 0 16 16\"><path d=\"M3 3h10v8H7l-3 2.5V11H3z\"/></symbol>\n  <symbol id=\"i-expand\" viewBox=\"0 0 16 16\"><path d=\"M9.5 6.5 13 3m0 0h-3m3 0v3M6.5 9.5 3 13m0 0h3m-3 0v-3\"/></symbol>\n  <symbol id=\"i-hide\" viewBox=\"0 0 16 16\"><path d=\"M2 8s2.2-4 6-4 6 4 6 4-2.2 4-6 4-6-4-6-4zM3 13 13 3\"/></symbol>\n  <symbol id=\"i-doc\" viewBox=\"0 0 16 16\"><rect x=\"3\" y=\"2\" width=\"10\" height=\"12\" rx=\"1.5\"/><path d=\"M5.5 6h5M5.5 9h5\"/></symbol>\n  <symbol id=\"i-find\" viewBox=\"0 0 16 16\"><circle cx=\"7\" cy=\"7\" r=\"4\"/><path d=\"m10 10 3.5 3.5\"/></symbol>\n  <symbol id=\"i-more\" viewBox=\"0 0 16 16\"><path d=\"M3.5 8h.01M8 8h.01M12.5 8h.01\" stroke-width=\"2.2\"/></symbol>\n  <symbol id=\"i-up\" viewBox=\"0 0 16 16\"><path d=\"M8 13V3M4 7l4-4 4 4\"/></symbol>\n  <symbol id=\"i-dn\" viewBox=\"0 0 16 16\"><path d=\"M8 3v10M4 9l4 4 4-4\"/></symbol>\n  <symbol id=\"i-person\" viewBox=\"0 0 16 16\"><circle cx=\"8\" cy=\"5\" r=\"2.5\"/><path d=\"M3 13.5c.6-2.6 2.6-4 5-4s4.400 1.400 5 4\"/></symbol>\n  <symbol id=\"i-moon\" viewBox=\"0 0 16 16\"><path d=\"M13 9.5A5.5 5.5 0 0 1 6.5 3 5.5 5.5 0 1 0 13 9.5z\"/></symbol>\n</svg>");

  var root = document.documentElement;
  function fit() {
    document.querySelectorAll(".stage").forEach(function (stage) {
      var scale = stage.querySelector(".scale");
      if (scale) {
        var W = +stage.dataset.w || 1280, H = +stage.dataset.h || 800; scale.style.width = W + "px"; scale.style.height = H + "px";
        var k = Math.min(1, stage.clientWidth / W);
        scale.style.transform = "scale(" + k + ")";
        stage.style.height = H * k + "px"; stage.style.maxWidth = W + "px";
      }
      stage.querySelectorAll(".pin").forEach(function (p) { p.remove(); });
      var box = stage.getBoundingClientRect();
      stage.querySelectorAll("[data-pin]").forEach(function (el) {
        var r = el.getBoundingClientRect(), pin = document.createElement("span");
        if (!r.width) return;
        pin.className = "pin"; pin.textContent = el.dataset.pin; pin.setAttribute("aria-hidden", "true");
        pin.style.left = Math.max(10, r.left - box.left - 15) + "px"; pin.style.top = Math.max(10, r.top - box.top + Math.min(r.height, 22) / 2) + "px";
        stage.appendChild(pin);
      });
    });
  }
  document.querySelector(".switches").addEventListener("change", function (e) {
    if (e.target.name === "theme") applyTheme(e.target.value); else if (e.target.closest(".switches") && e.target.name) root.dataset[e.target.name] = e.target.value;
    fit();
  });
  addEventListener("resize", fit);
  function applyTheme(name) {
    var th = THEMES[name]; if (!th) return;
    Object.keys(th).forEach(function (k) { if (k[0] !== "_") root.style.setProperty("--" + k, th[k]); });
    root.dataset.mode = th._mode;
    document.querySelectorAll("[data-theme-name]").forEach(function (el) { el.setAttribute("aria-pressed", el.dataset.themeName === name); });
    var sel = document.querySelector("select[name=theme]"); if (sel) sel.value = name;
    document.querySelectorAll("[data-theme-label]").forEach(function (el) { el.textContent = name; });
  }
  window.applyTheme = applyTheme;
  var sel = document.querySelector("select[name=theme]");
  if (sel) { Object.keys(THEMES).forEach(function (n) { var o = document.createElement("option"); o.textContent = n; sel.appendChild(o); }); }
  applyTheme("Loopflow Light");
  document.fonts.ready.then(fit); fit();
  function flash(el) { el.classList.add("on"); el.scrollIntoView({ block: "center", behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" }); setTimeout(function () { el.classList.remove("on"); }, 1800); }
  document.querySelectorAll("ol.notes").forEach(function (list) {
    var pair = list.previousElementSibling;
    list.addEventListener("click", function (e) {
      var li = e.target.closest("li"); if (!li) return;
      var n = li.querySelector("i").textContent;
      pair.querySelectorAll(".pin").forEach(function (p) { if (p.textContent === n) flash(p); });
    });
    pair.addEventListener("click", function (e) {
      if (!e.target.classList.contains("pin")) return;
      list.querySelectorAll("li").forEach(function (li) { if (li.querySelector("i").textContent === e.target.textContent) flash(li); });
    });
  });
  document.querySelectorAll(".tabs").forEach(function (tabs) {
    tabs.addEventListener("click", function (e) {
      var b = e.target.closest("[data-tab]"); if (!b) return;
      tabs.querySelectorAll("[data-tab]").forEach(function (x) { x.setAttribute("aria-selected", x === b); });
      tabs.closest(".pane").querySelectorAll("[data-panel]").forEach(function (x) { x.hidden = x.dataset.panel !== b.dataset.tab; }); fit();
    });
  });
  document.querySelectorAll(".term").forEach(function (x) { x.scrollTop = x.scrollHeight; });
  var go = document.getElementById("todialog"), term = document.querySelector(".term");
  if (go) go.addEventListener("click", function () { term.scrollTop = term.scrollHeight; term.focus(); });
  document.querySelectorAll(".app a").forEach(function (a) { a.addEventListener("click", function (e) { e.preventDefault(); }); });
