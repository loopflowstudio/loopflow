#if os(macOS)
import SwiftUI
import WebKit

/// The workspace's live web views, one per URL tab. A page keeps its history
/// across re-renders and pane moves until its tab closes.
@MainActor
final class BrowserPages {
    private var views: [String: WKWebView] = [:]

    func view(for id: String, url: URL) -> WKWebView {
        if let view = views[id] { return view }
        let view = WKWebView()
        view.allowsBackForwardNavigationGestures = true
        view.load(URLRequest(url: url))
        views[id] = view
        return view
    }

    func release(_ id: String) { views[id] = nil }
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
    @State private var title = ""

    var body: some View {
        HStack(spacing: 4) {
            Button { page.goBack() } label: { Image(systemName: "chevron.left") }
                .disabled(!canGoBack).help("Back").accessibilityIdentifier("browser-back")
            Button { page.goForward() } label: { Image(systemName: "chevron.right") }
                .disabled(!canGoForward).help("Forward").accessibilityIdentifier("browser-forward")
            Button { page.reload() } label: { Image(systemName: "arrow.clockwise") }
                .help("Reload").accessibilityIdentifier("browser-reload")
            Text(title).lineLimit(1).truncationMode(.middle).foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .buttonStyle(.borderless)
        .padding(.horizontal, 6).padding(.vertical, 3)
        .onReceive(page.publisher(for: \.canGoBack)) { canGoBack = $0 }
        .onReceive(page.publisher(for: \.canGoForward)) { canGoForward = $0 }
        .onReceive(page.publisher(for: \.url)) { title = $0?.absoluteString ?? "" }
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
