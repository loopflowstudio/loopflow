#if os(macOS)
import SwiftUI
import Synchronization
import WebKit

/// The workspace's live web views, one per URL tab. A page keeps its history
/// across re-renders, pane moves and the one close retained for Undo.
@MainActor
final class BrowserPages {
    private var views: [String: BrowserPage] = [:]
    private let lifetime: BrowserWorkspaceLifetime

    init(workspace: String = UUID().uuidString, log: DesktopLog = .home) {
        lifetime = BrowserWorkspaceLifetime(workspace: workspace, log: log)
    }

    deinit { lifetime.end() }

    func view(for id: String, url: URL) -> WKWebView {
        if let view = views[id] { return view }
        let view = BrowserPage(lifetime: BrowserPageLifetime(workspace: lifetime, tab: id, url: url))
        view.allowsBackForwardNavigationGestures = true
        view.lifetime.event("load-request")
        view.load(URLRequest(url: url))
        views[id] = view
        return view
    }

    func release(_ id: String) {
        guard let view = views.removeValue(forKey: id) else { return }
        view.lifetime.retire(reason: "undo-replaced")
    }
}

/// Only diagnostic state crosses deinit's actor boundary; no native views do.
private final class BrowserWorkspaceLifetime: Sendable {
    let workspace: String
    let instance = UUID().uuidString
    let log: DesktopLog
    private let ended = Mutex(false)

    init(workspace: String, log: DesktopLog) {
        self.workspace = workspace
        self.log = log
    }

    var isEnded: Bool { ended.withLock { $0 } }

    func end() {
        ended.withLock { $0 = true }
        log.info("browser workspace released", ["workspace": workspace, "pool": instance])
    }
}

/// Contains no URL or native view, so teardown logs cannot expose navigation data
/// or keep a page alive. URL schemes are the only address context recorded.
private final class BrowserPageLifetime: Sendable {
    let workspace: BrowserWorkspaceLifetime
    let tab: String
    let page = UUID().uuidString
    let scheme: String
    private let created = ProcessInfo.processInfo.systemUptime
    private let retirement = Mutex<String?>(nil)

    init(workspace: BrowserWorkspaceLifetime, tab: String, url: URL) {
        self.workspace = workspace
        self.tab = tab
        scheme = url.scheme?.lowercased() ?? "none"
    }

    func retire(reason: String) {
        retirement.withLock { $0 = reason }
        event("retire")
    }

    func event(_ event: String) {
        let reason = retirement.withLock { $0 } ?? (workspace.isEnded ? "workspace-loss" : "live")
        workspace.log.info("browser page", [
            "event": event, "workspace": workspace.workspace, "pool": workspace.instance,
            "tab": tab, "page": page, "scheme": scheme, "reason": reason,
            "age_ms": String(Int((ProcessInfo.processInfo.systemUptime - created) * 1000)),
        ])
    }
}

private final class BrowserPage: WKWebView, WKNavigationDelegate {
    let lifetime: BrowserPageLifetime

    init(lifetime: BrowserPageLifetime) {
        self.lifetime = lifetime
        super.init(frame: .zero, configuration: WKWebViewConfiguration())
        navigationDelegate = self
        lifetime.event("create")
    }

    required init?(coder: NSCoder) { fatalError("Browser pages are created by their workspace") }

    // Marks the native view entering destruction, not acknowledgement that
    // WebKit has drained its pending layer-tree commits.
    deinit { lifetime.event("final-release") }

    override func viewWillMove(toSuperview newSuperview: NSView?) {
        lifetime.event(newSuperview == nil ? "detach" : "attach")
        super.viewWillMove(toSuperview: newSuperview)
    }

    func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) {
        lifetime.event("load-start")
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        lifetime.event("load-finish")
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        lifetime.event("load-failed")
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        lifetime.event("load-failed")
    }

    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        lifetime.event("web-content-terminated")
    }
}

/// A web page in the browser pane. Only http and https reach it (`BrowserTab.page`).
struct BrowserPageView: View {
    let pages: BrowserPages
    let id: String
    let url: URL

    var body: some View {
        let page = pages.view(for: id, url: url)
        VStack(spacing: 0) {
            BrowserPageControls(page: page)
            Divider()
            BrowserPageSurface(page: page)
        }
    }
}

/// Back, Forward and Reload, enabled from the page's own history.
private struct BrowserPageControls: View {
    let page: WKWebView
    @State private var canGoBack = false
    @State private var canGoForward = false
    @State private var address = ""

    var body: some View {
        HStack(spacing: 4) {
            Button { page.goBack() } label: { Image(systemName: "chevron.left") }
                .disabled(!canGoBack).help("Back").accessibilityIdentifier("browser-back")
            Button { page.goForward() } label: { Image(systemName: "chevron.right") }
                .disabled(!canGoForward).help("Forward").accessibilityIdentifier("browser-forward")
            Button { page.reload() } label: { Image(systemName: "arrow.clockwise") }
                .help("Reload").accessibilityIdentifier("browser-reload")
            Text(address).lineLimit(1).truncationMode(.middle).foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .buttonStyle(.borderless)
        .padding(.horizontal, 6).padding(.vertical, 3)
        .onReceive(page.publisher(for: \.canGoBack)) { canGoBack = $0 }
        .onReceive(page.publisher(for: \.canGoForward)) { canGoForward = $0 }
        .onReceive(page.publisher(for: \.url)) { address = $0?.absoluteString ?? "" }
    }
}

private struct BrowserPageSurface: NSViewRepresentable {
    let page: WKWebView

    func makeNSView(context: Context) -> NSView {
        let container = NSView()
        updateNSView(container, context: context)
        return container
    }

    /// The retained page moves into whichever container shows it.
    func updateNSView(_ container: NSView, context: Context) {
        guard page.superview !== container else { return }
        page.removeFromSuperview()
        page.frame = container.bounds
        page.autoresizingMask = [.width, .height]
        container.addSubview(page)
    }
}
#endif
