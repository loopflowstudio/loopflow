const mobileDocs = window.matchMedia("(max-width: 760px)");

function syncDocsNavigation() {
  document.querySelectorAll(".docs-nav-disclosure").forEach((disclosure) => {
    if (mobileDocs.matches) {
      disclosure.removeAttribute("open");
    } else {
      disclosure.setAttribute("open", "");
    }
  });
}

syncDocsNavigation();
mobileDocs.addEventListener("change", syncDocsNavigation);
