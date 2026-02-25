import SwiftUI
import LoopflowCore

struct ShortcutHelpOverlay: View {
    @Binding var isPresented: Bool
    let shortcuts: [ShortcutBinding]
    let chords: [ChordBinding]

    @Environment(\.palette) private var palette
    @FocusState private var isCloseFocused: Bool

    private let columnLayout: [[ShortcutCategory]] = [
        [.navigation, .waveActions],
        [.global, .tools],
        [.tabs],
    ]

    private var entriesByCategory: [ShortcutCategory: [ShortcutEntry]] {
        var grouped: [ShortcutCategory: [ShortcutEntry]] = [:]
        for shortcut in shortcuts {
            grouped[shortcut.category, default: []].append(
                ShortcutEntry(key: shortcut.gesture.displayKey, label: shortcut.label)
            )
        }
        for chord in chords {
            grouped[chord.category, default: []].append(
                ShortcutEntry(key: chord.displayKey, label: chord.label)
            )
        }
        return grouped
    }

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
                    ForEach(columnLayout, id: \.self) { categories in
                        VStack(alignment: .leading, spacing: Spacing.lg) {
                            ForEach(categories, id: \.self) { category in
                                let entries = entriesByCategory[category, default: []]
                                if !entries.isEmpty {
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
                .foregroundStyle(palette.textSecondary)

            VStack(alignment: .leading, spacing: Spacing.xs) {
                ForEach(entries) { entry in
                    HStack(spacing: Spacing.sm) {
                        Text(entry.key)
                            .font(Typography.code(12))
                            .frame(width: 48, alignment: .leading)
                        Text(entry.label)
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                    }
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel("\(entry.key), \(entry.label)")
                }
            }
        }
    }
}

private struct ShortcutEntry: Identifiable {
    let key: String
    let label: String

    var id: String {
        "\(key)|\(label)"
    }
}

