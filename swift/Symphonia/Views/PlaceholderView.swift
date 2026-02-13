// Placeholder view for Symphonia teams app.
// Uses design token values directly since Symphonia doesn't depend on Concerto's DesignSystem.

import SwiftUI

struct PlaceholderView: View {
    var body: some View {
        VStack(spacing: 24) { // Spacing.xxl
            Image(systemName: "person.3.fill")
                .font(.system(size: 48))
                .foregroundStyle(.secondary)

            Text("Loopflow Symphonia")
                .font(.custom("Cormorant Garamond", size: 32).weight(.bold))

            Text("Teams coordination for LLM coding waves")
                .font(.custom("Lato", size: 14))
                .foregroundStyle(.secondary)

            Text("Coming soon")
                .font(.custom("Lato", size: 12))
                .foregroundStyle(.tertiary)
                .padding(.top, 16) // Spacing.lg
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

#Preview {
    PlaceholderView()
}
