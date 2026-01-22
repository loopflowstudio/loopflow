// Placeholder view for Symphonia teams app.

import SwiftUI

struct PlaceholderView: View {
    var body: some View {
        VStack(spacing: 24) {
            Image(systemName: "person.3.fill")
                .font(.system(size: 64))
                .foregroundStyle(.secondary)

            Text("Loopflow Symphonia")
                .font(.largeTitle)
                .fontWeight(.bold)

            Text("Teams coordination for LLM coding agents")
                .font(.title3)
                .foregroundStyle(.secondary)

            Text("Coming soon")
                .font(.headline)
                .foregroundStyle(.tertiary)
                .padding(.top, 16)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

#Preview {
    PlaceholderView()
}
