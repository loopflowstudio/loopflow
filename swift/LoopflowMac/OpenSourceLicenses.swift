import AppKit
import SwiftUI

struct AboutLoopflowView: View {
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        VStack(spacing: 12) {
            Image(nsImage: NSApplication.shared.applicationIconImage)
                .resizable()
                .frame(width: 64, height: 64)
                .accessibilityHidden(true)
            Text("Loopflow").font(.title.bold())
            Text(Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "Development")
                .foregroundStyle(.secondary)
            Button("Open Source Licenses") {
                openWindow(id: "open-source-licenses")
            }
        }
        .padding(32)
        .frame(width: 280)
    }
}

enum OpenSourceLicenses {
    static func text(in bundle: Bundle) throws -> String {
        guard let url = bundle.url(forResource: "NOTICES", withExtension: "txt", subdirectory: "ThirdPartyNotices") else {
            throw CocoaError(.fileNoSuchFile)
        }
        return try String(contentsOf: url, encoding: .utf8)
    }

    static var resourceBundle: Bundle {
        if Bundle.main.url(forResource: "NOTICES", withExtension: "txt", subdirectory: "ThirdPartyNotices") != nil {
            return .main
        }
        #if SWIFT_PACKAGE
        return .module
        #else
        return .main
        #endif
    }
}

struct OpenSourceLicensesView: View {
    var body: some View {
        Group {
            if let text = try? OpenSourceLicenses.text(in: OpenSourceLicenses.resourceBundle) {
                LicenseTextView(text: text)
            } else {
                ContentUnavailableView("Licenses could not be loaded", systemImage: "doc.text",
                                       description: Text("This app is missing its bundled license notices. Reinstall Loopflow to restore them."))
            }
        }
        .frame(minWidth: 600, minHeight: 400)
    }
}

private struct LicenseTextView: NSViewRepresentable {
    let text: String

    func makeNSView(context: Context) -> NSScrollView {
        let scroll = NSTextView.scrollableTextView()
        guard let view = scroll.documentView as? NSTextView else { return scroll }
        view.isEditable = false
        view.isSelectable = true
        view.usesFindBar = true
        view.isIncrementalSearchingEnabled = true
        view.font = .systemFont(ofSize: NSFont.systemFontSize)
        view.textContainerInset = NSSize(width: 20, height: 20)
        view.setAccessibilityLabel("Open Source Licenses")
        view.string = text
        return scroll
    }

    func updateNSView(_ nsView: NSScrollView, context: Context) {}
}
