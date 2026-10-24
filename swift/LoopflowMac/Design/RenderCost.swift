import Observation
import SwiftUI

// MARK: - Hover

/// Hover that a container reports and only its revealed leaves read. Crossing
/// the container re-renders those leaves, never the container: the writer's
/// body does not read `isHovered`.
@MainActor
@Observable
final class HoverFlag {
    var isHovered = false
}

extension View {
    @MainActor func hoverSource(_ flag: HoverFlag) -> some View {
        onHover { inside in if flag.isHovered != inside { flag.isHovered = inside } }
    }
}

/// Always in the tree, so a hover changes opacity rather than inserting views.
/// Shown while hovered, or with `whileHovered: false` while not hovered; hidden
/// content takes no clicks and is not read aloud.
struct HoverRevealed<Content: View>: View {
    let flag: HoverFlag
    var whileHovered = true
    @ViewBuilder let content: Content

    var body: some View {
        let shown = flag.isHovered == whileHovered
        content.opacity(shown ? 1 : 0)
            .animation(.easeOut(duration: 0.12), value: shown)
            .allowsHitTesting(shown)
            .accessibilityHidden(!shown)
            .renderFlash()
    }
}

// MARK: - Render flash

/// Dev builds only: Debug › Flash Re-renders tints each instrumented view with
/// a new color whenever its body runs, so a steady color means no re-render.
@MainActor
@Observable
final class RenderFlash {
    static let shared = RenderFlash()
    static let available = Bundle.main.bundleIdentifier?.hasSuffix(".dev") == true
    private static let key = "RenderFlash"

    private(set) var enabled = available && UserDefaults.standard.bool(forKey: key)

    func setEnabled(_ enabled: Bool) {
        self.enabled = Self.available && enabled
        UserDefaults.standard.set(self.enabled, forKey: Self.key)
    }
}

extension View {
    /// Call inside the body being measured: the tint is chosen when that body runs.
    @MainActor func renderFlash() -> some View {
        overlay {
            if RenderFlash.shared.enabled {
                Rectangle()
                    .fill(Color(hue: .random(in: 0...1), saturation: 0.9, brightness: 0.95).opacity(0.22))
                    .allowsHitTesting(false)
                    .accessibilityHidden(true)
            }
        }
    }
}
