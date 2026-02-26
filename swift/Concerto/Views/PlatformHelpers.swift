#if os(macOS)
import SwiftUI
import AppKit

func copyToClipboard(_ content: String) {
    NSPasteboard.general.clearContents()
    NSPasteboard.general.setString(content, forType: .string)
}

func openMicrophoneSettings() {
    if let privacyURL = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone") {
        NSWorkspace.shared.open(privacyURL)
        return
    }
    if let settingsURL = URL(string: "x-apple.systempreferences:com.apple.systempreferences") {
        NSWorkspace.shared.open(settingsURL)
    }
}

extension View {
    @ViewBuilder
    func hoverTracking(_ onChange: @escaping (Bool) -> Void) -> some View {
        self.onHover(perform: onChange)
    }

    func macOSFocusable() -> some View {
        self.focusable()
    }

    func macOSOnExitCommand(perform action: @escaping () -> Void) -> some View {
        self.onExitCommand(perform: action)
    }
}

#elseif canImport(UIKit)
import SwiftUI
import UIKit

func copyToClipboard(_ content: String) {
    UIPasteboard.general.string = content
}

func openMicrophoneSettings() {
    guard let settingsURL = URL(string: UIApplication.openSettingsURLString),
          UIApplication.shared.canOpenURL(settingsURL) else {
        return
    }
    UIApplication.shared.open(settingsURL)
}

extension View {
    @ViewBuilder
    func hoverTracking(_ onChange: @escaping (Bool) -> Void) -> some View {
        self
    }

    func macOSFocusable() -> some View {
        self
    }

    func macOSOnExitCommand(perform action: @escaping () -> Void) -> some View {
        self
    }
}
#endif
