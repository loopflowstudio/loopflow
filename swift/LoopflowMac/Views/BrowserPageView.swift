#if os(macOS)
import SwiftUI
import WebKit

/// A web page in the browser pane. Only http and https reach it (`BrowserTab.page`).
struct BrowserPageView: NSViewRepresentable {
    let url: URL

    func makeNSView(context: Context) -> WKWebView {
        let view = WKWebView()
        view.allowsBackForwardNavigationGestures = true
        view.load(URLRequest(url: url))
        return view
    }

    func updateNSView(_ view: WKWebView, context: Context) {
        if view.url == nil { view.load(URLRequest(url: url)) }
    }
}
#endif
