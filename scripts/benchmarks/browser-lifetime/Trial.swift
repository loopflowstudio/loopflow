import AppKit
import Darwin
import MachO
import WebKit

private func emit(_ event: String, _ fields: [String: Any] = [:]) {
    var fields = fields
    fields["event"] = event
    fields["uptime_ms"] = ProcessInfo.processInfo.systemUptime * 1000
    guard var bytes = try? JSONSerialization.data(withJSONObject: fields, options: [.sortedKeys]) else { return }
    bytes.append(10)
    try? FileHandle.standardOutput.write(contentsOf: bytes)
}

private func webKitImage() -> [String: String] {
    let bundle = Bundle(for: WKWebView.self)
    var result = ["bundle": bundle.bundlePath,
                  "version": bundle.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "unknown"]
    for index in 0..<_dyld_image_count() {
        guard let name = _dyld_get_image_name(index), String(cString: name).hasSuffix("/WebKit"),
              let header = _dyld_get_image_header(index) else { continue }
        result["image"] = String(cString: name)
        var command = UnsafeRawPointer(header).advanced(by: MemoryLayout<mach_header_64>.size)
        for _ in 0..<header.pointee.ncmds {
            let load = command.load(as: load_command.self)
            if load.cmd == LC_UUID {
                result["uuid"] = UUID(uuid: command.load(as: uuid_command.self).uuid).uuidString
            }
            command = command.advanced(by: Int(load.cmdsize))
        }
    }
    return result
}

private final class PageLifetime: Sendable {
    let id: String
    init(_ id: String) { self.id = id }
    deinit { emit("final-release", ["page": id]) }
}

private final class TrialPage: WKWebView {
    let lifetime: PageLifetime
    init(_ id: String, configuration: WKWebViewConfiguration) {
        lifetime = PageLifetime(id)
        super.init(frame: .zero, configuration: configuration)
    }
    required init?(coder: NSCoder) { fatalError("No archived pages") }
}

/// Each process owns one disposable window, no Desktop or Loopflow store.
@MainActor
private final class Trial: NSObject, NSApplicationDelegate, WKNavigationDelegate, WKScriptMessageHandler {
    let variant: String
    let scenario: String
    let offset: Int
    let hold: Int
    let settle: Int
    let window = NSWindow(contentRect: NSRect(x: 100, y: 100, width: 800, height: 600),
                          styleMask: [.titled], backing: .buffered, defer: false)
    var pages: [String: TrialPage] = [:]
    var undo: TrialPage?
    var retired: [String: TrialPage] = [:]
    var references: [String: WeakPage] = [:]
    var scheduled = false
    var started = false
    var navigationFinished: Double?

    init(variant: String, scenario: String, offset: Int, hold: Int, settle: Int) {
        self.variant = variant
        self.scenario = scenario
        self.offset = offset
        self.hold = hold
        self.settle = settle
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        window.title = "Browser lifetime diagnostic — disposable"
        window.isReleasedWhenClosed = false
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        let page = makePage("first")
        mount(page)
        if scenario == "two-close" || scenario == "workspace" {
            let second = makePage("second")
            second.loadHTMLString(Self.html, baseURL: nil)
        }
        // Identical local content for all variants; no network or machine data.
        page.loadHTMLString(Self.html, baseURL: URL(string: "https://browser-lifetime.invalid/"))
        DispatchQueue.main.asyncAfter(deadline: .now() + 15) { [weak self] in
            guard let self, !self.started else { return }
            emit("incomplete", ["reason": "navigation-never-finished"])
            exit(2)
        }
    }

    func makePage(_ id: String) -> TrialPage {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        configuration.userContentController.add(self, name: "paint")
        let page = TrialPage(id, configuration: configuration)
        page.navigationDelegate = self
        pages[id] = page
        references[id] = WeakPage(page)
        emit("create", ["page": id])
        return page
    }

    func mount(_ page: TrialPage) {
        let container = NSView(frame: window.contentView?.bounds ?? .zero)
        window.contentView = container
        page.removeFromSuperview()
        page.frame = container.bounds
        page.autoresizingMask = [.width, .height]
        container.addSubview(page)
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        guard let page = webView as? TrialPage else { return }
        let id = page.lifetime.id
        emit("navigation-finish", ["page": id, "blank": retired[id] != nil])
        if variant == "blank", retired[id] != nil {
            finishRetirement(id, reason: "blank-finish")
            return
        }
        guard id == "first", !scheduled else { return }
        scheduled = true
        navigationFinished = ProcessInfo.processInfo.systemUptime * 1000
        // didFinish is a navigation anchor, never a first-paint acknowledgement.
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(offset)) { [weak self] in self?.exercise() }
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        failed(webView)
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        failed(webView)
    }

    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        emit("web-content-terminated")
        failed(webView)
    }

    func failed(_ webView: WKWebView) {
        guard let page = webView as? TrialPage else { return }
        emit("navigation-failed", ["page": page.lifetime.id])
        if retired[page.lifetime.id] != nil { finishRetirement(page.lifetime.id, reason: "navigation-failed") }
    }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard let body = message.body as? [String: Any] else { return }
        emit("paint-observation", body)
    }

    func close(_ id: String) {
        let replaced = undo
        undo = pages.removeValue(forKey: id)
        undo?.removeFromSuperview()
        if let replaced { dispose(replaced) }
    }

    func exercise() {
        started = true
        emit("exercise", ["scenario": scenario, "offset_ms": offset])
        switch scenario {
        case "two-close":
            close("first")
            if let second = pages["second"] { mount(second) }
            close("second")
            pages["second"] = undo // Undo must restore the very same page.
            undo = nil
            emit("undo", ["same_page": pages["second"] === references["second"]?.page])
        case "shell-replace", "pane-replace":
            close("first")
            if let undo { dispose(undo) }
            undo = nil // A shell close or pane hide displaces browser Undo.
        case "reparent":
            if let first = pages["first"] { mount(first) }
            emit("reparent", ["same_page": pages["first"] === references["first"]?.page])
            if let first = pages.removeValue(forKey: "first") { dispose(first) }
        case "workspace":
            close("first")
            let owned = Array(pages.values) + [undo].compactMap { $0 }
            pages.removeAll()
            undo = nil
            for page in owned { dispose(page) }
        default:
            if let first = pages.removeValue(forKey: "first") { dispose(first) }
        }
        emit("ownership", ["open": pages.count, "undo": undo == nil ? 0 : 1, "retiring": retired.count])
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(settle)) { [weak self] in self?.cleanup() }
    }

    func dispose(_ page: TrialPage) {
        let id = page.lifetime.id
        emit("retire", ["page": id, "variant": variant,
                        "since_navigation_ms": navigationFinished.map { ProcessInfo.processInfo.systemUptime * 1000 - $0 } ?? -1])
        page.removeFromSuperview()
        if variant != "immediate" { page.stopLoading() }
        if variant == "deferred" || variant == "blank" {
            retired[id] = page
            if variant == "blank" { page.load(URLRequest(url: URL(string: "about:blank")!)) }
            DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(hold)) { [weak self] in
                self?.finishRetirement(id, reason: "experimental-bound")
            }
        }
    }

    func finishRetirement(_ id: String, reason: String) {
        guard let page = retired.removeValue(forKey: id) else { return }
        page.stopLoading()
        emit("retirement-finished", ["page": id, "reason": reason])
    }

    func cleanup() {
        // Undo/open pages survive the measured checkpoint, then the harness
        // releases them too so a leaked handler/container cannot hide reclamation.
        for page in pages.values { page.removeFromSuperview(); page.stopLoading() }
        undo?.stopLoading()
        pages.removeAll()
        undo = nil
        window.contentView = nil
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(settle)) { [weak self] in
            guard let self else { return }
            let retained = self.references.values.filter { $0.page != nil }.count
            emit("complete", ["retained": retained, "retiring": self.retired.count, "created": self.references.count])
            exit(retained == 0 && self.retired.isEmpty ? 0 : 3)
        }
    }

    static let html = """
    <!doctype html><meta charset="utf-8"><style>
    body { background:#15243c; color:white; font:32px system-ui; }
    div { width:400px; height:240px; transform:translateZ(0); background:linear-gradient(45deg,#f68,#48f); }
    </style><h1>Local first-paint lifetime trial</h1><div></div><canvas width="600" height="200"></canvas>
    <script>
    const send = value => window.webkit.messageHandlers.paint.postMessage(value);
    const supported = PerformanceObserver.supportedEntryTypes?.includes('paint') ?? false;
    send({kind:'paint-support', supported});
    if (supported) new PerformanceObserver(list => list.getEntries().forEach(entry =>
      send({kind:entry.name, page_ms:entry.startTime}))).observe({type:'paint', buffered:true});
    requestAnimationFrame(() => requestAnimationFrame(() => send({kind:'animation-frame', page_ms:performance.now()})));
    const ctx = document.querySelector('canvas').getContext('2d');
    ctx.fillStyle='#fea'; ctx.fillRect(0,0,600,200);
    </script>
    """
}

@MainActor
private final class WeakPage {
    weak var page: TrialPage?
    init(_ page: TrialPage) { self.page = page }
}

let arguments = CommandLine.arguments
let session = CGSessionCopyCurrentDictionary() as? [String: Any]
let display = session?[kCGSessionOnConsoleKey as String] as? Bool == true && CGMainDisplayID() != 0
emit("environment", ["os": ProcessInfo.processInfo.operatingSystemVersionString,
                     "webkit": webKitImage(), "display_session": display, "pid": getpid()])
if arguments.contains("--check") { exit(display ? 0 : 2) }
guard display, arguments.count == 6 else {
    emit("incomplete", ["reason": "requires-display-session-and-five-arguments"])
    exit(2)
}
let app = NSApplication.shared
app.setActivationPolicy(.regular)
private let trial = Trial(variant: arguments[1], scenario: arguments[2], offset: Int(arguments[3])!,
                  hold: Int(arguments[4])!, settle: Int(arguments[5])!)
app.delegate = trial
app.run()
