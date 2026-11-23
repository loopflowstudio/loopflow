#if os(macOS)
import AppKit
import SwiftUI
import WebKit

/// Pages run their scripts but never reach the network; links open in the browser.
/// A rewrite reloads in place at the same scroll position.
struct TaskHTMLView: View {
    let html: String
    let baseURL: URL?
    var find: FileFind?
    var pageIndex = 0
    var pageCount = 1
    var ready: () -> Void = {}

    @State private var page: WebPage?
    @State private var rulesUnavailable = false
    @State private var position = ScrollPosition()
    @State private var offset = CGPoint.zero
    /// Offsets during a load are the page settling, not the person scrolling.
    @State private var loading = false
    @State private var loaded = 0
    @State private var searched = false

    var body: some View {
        Group {
            if let page {
                WebView(page)
                    .webViewScrollPosition($position)
                    .webViewOnScrollGeometryChange(for: CGPoint.self, of: \.contentOffset) { _, new in
                        if !loading { offset = new }
                    }
            } else if rulesUnavailable {
                Text("Page unavailable: network blocking could not be set up.")
                    .task(id: find?.query) { find?.receive(page: pageIndex, count: 0, expected: pageCount) }
            } else {
                Color.clear
            }
        }
        .task(id: html) { await show() }
        .task(id: FileFind.Scan(query: find?.query, revision: loaded)) {
            guard let page, loaded > 0 else { return }
            let needle = find?.query ?? ""
            guard !needle.isEmpty || searched else { return }
            searched = !needle.isEmpty
            let count = try? await page.callJavaScript(Self.searchScript, arguments: ["needle": needle],
                                                       contentWorld: .defaultClient) as? Int
            guard let find, !needle.isEmpty, !Task.isCancelled, find.query == needle else { return }
            find.receive(page: pageIndex, count: count ?? 0, expected: pageCount)
            await revealMatch()
        }
        .task(id: find?.current) { await revealMatch() }
    }

    private func revealMatch() async {
        guard let page, let find, !find.query.isEmpty else { return }
        let index: Int
        if case .html(let side, let occurrence) = find.current?.cell, side == pageIndex { index = occurrence }
        else { index = -1 }
        _ = try? await page.callJavaScript(Self.revealScript, arguments: ["index": index, "needle": find.query],
                                           contentWorld: .defaultClient)
    }

    static let searchScript = #"""
    CSS.highlights.delete("lf-find");
    const ranges = [];
    globalThis.fileFind = {needle, ranges};
    if (!needle || !document.body) return 0;
    if (!globalThis.fileFindStyle) {
        const style = new CSSStyleSheet();
        style.replaceSync("::highlight(lf-find) { background: Highlight; color: HighlightText }");
        document.adoptedStyleSheets = [...document.adoptedStyleSheets, style];
        globalThis.fileFindStyle = style;
    }
    const pattern = new RegExp(RegExp.escape(needle), "giu");
    // Inline markup can divide one word or phrase among several text nodes.
    let block, nodes = [], text = "";
    function flush() {
        let i = 0;
        for (const hit of text.matchAll(pattern)) {
            const start = hit.index, end = start + hit[0].length;
            while (nodes[i].end <= start) i++;
            const range = new Range();
            range.setStart(nodes[i].node, start - nodes[i].start);
            while (nodes[i].end < end) i++;
            range.setEnd(nodes[i].node, end - nodes[i].start);
            ranges.push(range);
        }
        nodes = [];
        text = "";
    }
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
        const node = walker.currentNode;
        if (node.nodeName === "BR") { flush(); continue; }
        if (node.nodeType !== Node.TEXT_NODE || !node.length) continue;
        let parent = node.parentElement;
        if (!parent?.checkVisibility({visibilityProperty: true, opacityProperty: true})) continue;
        while (parent.parentElement && ["inline", "contents"].includes(getComputedStyle(parent).display)) parent = parent.parentElement;
        if (parent !== block) { flush(); block = parent; }
        nodes.push({node, start: text.length, end: text.length + node.length});
        text += node.data;
    }
    flush();
    return ranges.length;
    """#

    static let revealScript = #"""
    const result = globalThis.fileFind;
    if (!result || result.needle !== needle) return;
    CSS.highlights.delete("lf-find");
    const range = result.ranges[index];
    if (!range) return;
    CSS.highlights.set("lf-find", new Highlight(range));
    range.startContainer.parentElement.scrollIntoView({block: "nearest"});
    const rect = range.getBoundingClientRect();
    if (rect.top < 0 || rect.bottom > innerHeight) scrollBy(0, rect.top - innerHeight / 2);
    """#

    private func show() async {
        if page != nil {
            do { try await Task.sleep(for: .milliseconds(800)) } catch { return }
        }
        if page == nil {
            guard let rules = await TaskHTMLRules.offline() else {
                rulesUnavailable = true
                return
            }
            guard !Task.isCancelled else { return }
            let configuration = WebPage.Configuration()
            configuration.userContentController.add(rules)
            page = WebPage(configuration: configuration, navigationDecider: TaskHTMLLinks())
        }
        guard let page else { return }
        let restore = offset
        loading = true
        defer { loading = false }
        guard await Self.load(page, html: html, baseURL: baseURL), !Task.isCancelled else { return }
        position.scrollTo(point: restore)
        loaded += 1
        ready()
    }

    /// Whether the page finished loading; a superseded or failed load reports false.
    @discardableResult
    private static func load(_ page: WebPage, html: String, baseURL: URL?) async -> Bool {
        do {
            for try await _ in page.load(html: html, baseURL: baseURL ?? URL(string: "about:blank")!) {}
            return true
        } catch { return false }
    }
}

/// Links leave the page for the browser; everything else loads in place.
private struct TaskHTMLLinks: WebPage.NavigationDeciding {
    func decidePolicy(for action: WebPage.NavigationAction,
                      preferences: inout WebPage.NavigationPreferences) async -> WKNavigationActionPolicy {
        guard action.navigationType == .linkActivated, let url = action.request.url,
              ["http", "https", "mailto"].contains(url.scheme?.lowercased() ?? "") else { return .allow }
        NSWorkspace.shared.open(url)
        return .cancel
    }
}

/// One compiled rule list blocks every network scheme for rendered pages.
@MainActor
private enum TaskHTMLRules {
    private static var compiled: Task<WKContentRuleList?, Never>?
    private static let rules = #"""
    [{"trigger":{"url-filter":"^https?://"},"action":{"type":"block"}},
     {"trigger":{"url-filter":"^wss?://"},"action":{"type":"block"}},
     {"trigger":{"url-filter":"^ftp://"},"action":{"type":"block"}}]
    """#

    static func offline() async -> WKContentRuleList? {
        let compiled = compiled ?? Task {
            try? await WKContentRuleListStore.default().compileContentRuleList(
                forIdentifier: "loopflow-offline-pages", encodedContentRuleList: rules)
        }
        Self.compiled = compiled
        return await compiled.value
    }
}
#endif
