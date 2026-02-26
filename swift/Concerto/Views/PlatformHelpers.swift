#if os(macOS)
import SwiftUI
import AppKit

func copyToClipboard(_ content: String) {
    NSPasteboard.general.clearContents()
    NSPasteboard.general.setString(content, forType: .string)
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
