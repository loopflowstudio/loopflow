// ThemePreview - renders content in light, dark, and deep wine side-by-side.
// Use in #Preview blocks to compare all three themes simultaneously.

import SwiftUI

struct ThemePreview<Content: View>: View {
    let content: () -> Content

    var body: some View {
        HStack(spacing: 0) {
            content()
                .environment(\.palette, .light)
                .environment(\.colorScheme, .light)
            content()
                .environment(\.palette, .dark)
                .environment(\.colorScheme, .dark)
            content()
                .environment(\.palette, .deepWine)
                .environment(\.colorScheme, .dark)
        }
    }
}
