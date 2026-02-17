import SwiftUI

struct ShortcutHelpOverlay: View {
    @Binding var isPresented: Bool
    let shortcuts: [ShortcutBinding]
    let chords: [ChordBinding]

    @FocusState private var isCloseFocused: Bool

    private let columnLayout: [[ShortcutCategory]] = [
        [.navigation, .waveActions],
        [.global, .tools],
        [.tabs],
    ]

    var body: some View {
        ZStack {
            Color.black.opacity(0.35)
                .ignoresSafeArea()
                .onTapGesture {
                    isPresented = false
                }

            VStack(alignment: .leading, spacing: Spacing.lg) {
                HStack {
                    Text("Keyboard Shortcuts")
                        .font(Typography.sectionTitle())
                        .fontWeight(.semibold)

                    Spacer()

                    Button {
                        isPresented = false
                    } label: {
                        Image(systemName: "xmark")
                            .font(Typography.caption())
                    }
                    .buttonStyle(.plain)
                    .focused($isCloseFocused)
                    .accessibilityLabel("Close keyboard shortcuts")
                }

                HStack(alignment: .top, spacing: Spacing.xxl) {
                    ForEach(Array(columnLayout.enumerated()), id: \.offset) { _, categories in
                        VStack(alignment: .leading, spacing: Spacing.lg) {
                            ForEach(categories, id: \.self) { category in
                                if let entries = entries(for: category), !entries.isEmpty {
                                    categorySection(title: category.rawValue, entries: entries)
                                }
                            }
                        }
                    }
                }
            }
            .padding(Spacing.xl)
            .frame(maxWidth: 760)
            .background(.ultraThinMaterial)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.xl))
            .shadow(color: .black.opacity(0.25), radius: 24, y: 12)
            .focusSection()
        }
        .zIndex(ZIndex.modal)
        .onAppear {
            isCloseFocused = true
        }
    }

    private func categorySection(title: String, entries: [ShortcutEntry]) -> some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text(title)
                .font(Typography.caption())
                .fontWeight(.semibold)
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: Spacing.xs) {
                ForEach(entries) { entry in
                    HStack(spacing: Spacing.sm) {
                        Text(entry.key)
                            .font(Typography.code(12))
                            .frame(width: 48, alignment: .leading)
                        Text(entry.label)
                            .font(Typography.caption())
                            .foregroundStyle(.secondary)
                    }
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel("\(entry.key), \(entry.label)")
                }
            }
        }
    }

    private func entries(for category: ShortcutCategory) -> [ShortcutEntry]? {
        var entries: [ShortcutEntry] = []

        entries.append(contentsOf: shortcuts
            .filter { $0.category == category }
            .map { ShortcutEntry(key: $0.gesture.displayKey, label: $0.label) }
        )

        entries.append(contentsOf: chords
            .filter { $0.category == category }
            .map { ShortcutEntry(key: $0.displayKey, label: $0.label) }
        )

        return entries
    }
}

private struct ShortcutEntry: Identifiable {
    let id = UUID()
    let key: String
    let label: String
}
